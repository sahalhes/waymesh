use crate::crypto::{generate_otp, hash_otp, verify_otp};
use crate::error::{CoreError, Result};
use crate::journey::GeoPoint;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ParcelStatus {
    Draft,
    Published,
    Matched,
    PickupPending,
    InTransit,
    Delivered,
    Returned,
    Disputed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parcel {
    pub id: Uuid,
    pub sender_id: Uuid,
    pub recipient_name: String,
    pub recipient_phone_hash: String,
    pub pickup_point: GeoPoint,
    pub dropoff_point: GeoPoint,
    pub pickup_address: String,
    pub dropoff_address: String,
    pub weight_kg: f64,
    pub volume_liters: f64,
    pub contents_declaration: String,
    pub prohibited_items_screened: bool,
    pub deadline_time: DateTime<Utc>,
    pub offered_compensation: u32, // minor currency units
    pub assigned_journey_id: Option<Uuid>,
    pub carrier_id: Option<Uuid>,
    pub pickup_otp_hash: Option<String>,
    pub delivery_otp_hash: Option<String>,
    pub failed_pickup_attempts: u32,
    pub failed_delivery_attempts: u32,
    pub status: ParcelStatus,
    pub picked_up_at: Option<DateTime<Utc>>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoverCodes {
    pub pickup_otp: String,
    pub delivery_otp: String,
}

impl Parcel {
    pub const MAX_OTP_ATTEMPTS: u32 = 3;

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        sender_id: Uuid,
        recipient_name: String,
        recipient_phone: &str,
        pickup_address: String,
        pickup_point: GeoPoint,
        dropoff_address: String,
        dropoff_point: GeoPoint,
        weight_kg: f64,
        volume_liters: f64,
        contents_declaration: String,
        deadline_time: DateTime<Utc>,
        offered_compensation: u32,
    ) -> Self {
        let recipient_phone_hash = crate::crypto::hash_otp(recipient_phone);
        Self {
            id: Uuid::new_v4(),
            sender_id,
            recipient_name,
            recipient_phone_hash,
            pickup_point,
            dropoff_point,
            pickup_address,
            dropoff_address,
            weight_kg,
            volume_liters,
            contents_declaration,
            prohibited_items_screened: true,
            deadline_time,
            offered_compensation,
            assigned_journey_id: None,
            carrier_id: None,
            pickup_otp_hash: None,
            delivery_otp_hash: None,
            failed_pickup_attempts: 0,
            failed_delivery_attempts: 0,
            status: ParcelStatus::Published,
            picked_up_at: None,
            delivered_at: None,
            created_at: Utc::now(),
        }
    }

    /// Carrier accepts the parcel: generates dual OTP codes
    pub fn accept(&mut self, carrier_id: Uuid, journey_id: Uuid) -> Result<HandoverCodes> {
        if self.sender_id == carrier_id {
            return Err(CoreError::SelfDealingProhibited);
        }

        if self.status != ParcelStatus::Published {
            return Err(CoreError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "PickupPending".into(),
            });
        }

        let pickup_otp = generate_otp();
        let delivery_otp = generate_otp();

        self.pickup_otp_hash = Some(hash_otp(&pickup_otp));
        self.delivery_otp_hash = Some(hash_otp(&delivery_otp));
        self.carrier_id = Some(carrier_id);
        self.assigned_journey_id = Some(journey_id);
        self.status = ParcelStatus::PickupPending;

        Ok(HandoverCodes {
            pickup_otp,
            delivery_otp,
        })
    }

    /// Physical pickup: carrier verifies pickup OTP provided by sender (enforces max 3 attempts)
    pub fn verify_pickup(&mut self, otp_input: &str) -> Result<()> {
        if self.status != ParcelStatus::PickupPending {
            return Err(CoreError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "InTransit".into(),
            });
        }
        if self.failed_pickup_attempts >= Self::MAX_OTP_ATTEMPTS {
            return Err(CoreError::OtpAttemptsExceeded);
        }
        let hash = self
            .pickup_otp_hash
            .as_ref()
            .ok_or(CoreError::OtpMismatch)?;
        if let Err(e) = verify_otp(otp_input, hash) {
            self.failed_pickup_attempts += 1;
            if self.failed_pickup_attempts >= Self::MAX_OTP_ATTEMPTS {
                self.status = ParcelStatus::Disputed;
                return Err(CoreError::OtpAttemptsExceeded);
            }
            return Err(e);
        }
        self.status = ParcelStatus::InTransit;
        self.failed_pickup_attempts = 0;
        self.picked_up_at = Some(Utc::now());
        Ok(())
    }

    /// Physical delivery: carrier verifies delivery OTP provided by recipient (enforces max 3 attempts)
    pub fn verify_delivery(&mut self, otp_input: &str) -> Result<()> {
        if self.status != ParcelStatus::InTransit {
            return Err(CoreError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "Delivered".into(),
            });
        }
        if self.failed_delivery_attempts >= Self::MAX_OTP_ATTEMPTS {
            return Err(CoreError::OtpAttemptsExceeded);
        }
        let hash = self
            .delivery_otp_hash
            .as_ref()
            .ok_or(CoreError::OtpMismatch)?;
        if let Err(e) = verify_otp(otp_input, hash) {
            self.failed_delivery_attempts += 1;
            if self.failed_delivery_attempts >= Self::MAX_OTP_ATTEMPTS {
                self.status = ParcelStatus::Disputed;
                return Err(CoreError::OtpAttemptsExceeded);
            }
            return Err(e);
        }
        self.status = ParcelStatus::Delivered;
        self.failed_delivery_attempts = 0;
        self.delivered_at = Some(Utc::now());
        Ok(())
    }

    pub fn cancel(&mut self) -> Result<()> {
        if self.status == ParcelStatus::InTransit || self.status == ParcelStatus::Delivered {
            return Err(CoreError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "Cancelled".into(),
            });
        }
        self.status = ParcelStatus::Cancelled;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parcel_dual_otp_custody_handover() {
        let mut parcel = Parcel::new(
            Uuid::new_v4(),
            "Aarav Sharma".into(),
            "+919876543210",
            "Campus Hostel B".into(),
            GeoPoint::new(12.9716, 77.5946),
            "Station Platform 1 Cloakroom".into(),
            GeoPoint::new(12.9780, 77.5696),
            2.5,
            3.0,
            "Engineering textbooks and notes".into(),
            Utc::now() + chrono::Duration::hours(4),
            12000,
        );

        let carrier_id = Uuid::new_v4();
        let journey_id = Uuid::new_v4();

        // Carrier accepts
        let codes = parcel
            .accept(carrier_id, journey_id)
            .expect("Should accept");
        assert_eq!(parcel.status, ParcelStatus::PickupPending);

        // Verification of pickup
        assert!(parcel.verify_pickup(&codes.pickup_otp).is_ok());
        assert_eq!(parcel.status, ParcelStatus::InTransit);

        // Delivery
        assert!(parcel.verify_delivery(&codes.delivery_otp).is_ok());
        assert_eq!(parcel.status, ParcelStatus::Delivered);
    }
}
