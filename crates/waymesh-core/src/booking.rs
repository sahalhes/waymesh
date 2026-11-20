use crate::crypto::{generate_otp, hash_otp, verify_otp};
use crate::error::{CoreError, Result};
use crate::identity::Gender;
use crate::journey::GeoPoint;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BookingStatus {
    Requested,
    Accepted,
    PickupConfirmed,
    Completed,
    Cancelled,
    Disputed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RideBooking {
    pub id: Uuid,
    pub journey_id: Uuid,
    pub passenger_id: Uuid,
    pub passenger_gender: Gender,
    pub seats_booked: u32,
    pub pickup_name: String,
    pub pickup_point: GeoPoint,
    pub dropoff_name: String,
    pub dropoff_point: GeoPoint,
    pub agreed_cost_share: u32,
    pub pickup_otp_hash: Option<String>,
    pub failed_pickup_attempts: u32,
    pub status: BookingStatus,
    pub confirmed_pickup_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl RideBooking {
    pub const MAX_OTP_ATTEMPTS: u32 = 3;

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        journey_id: Uuid,
        passenger_id: Uuid,
        passenger_gender: Gender,
        seats_booked: u32,
        pickup_name: String,
        pickup_point: GeoPoint,
        dropoff_name: String,
        dropoff_point: GeoPoint,
        agreed_cost_share: u32,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            journey_id,
            passenger_id,
            passenger_gender,
            seats_booked,
            pickup_name,
            pickup_point,
            dropoff_name,
            dropoff_point,
            agreed_cost_share,
            pickup_otp_hash: None,
            failed_pickup_attempts: 0,
            status: BookingStatus::Requested,
            confirmed_pickup_at: None,
            completed_at: None,
            created_at: Utc::now(),
        }
    }

    /// Validates gender compatibility between passenger and driver.
    /// Strict protocol rule: In ride matching, do not match male and female.
    pub fn validate_gender_compatibility(&self, driver_gender: &Gender) -> Result<()> {
        if !self.passenger_gender.is_compatible_for_ride(driver_gender) {
            return Err(CoreError::GenderIncompatible);
        }
        Ok(())
    }

    /// Driver accepts the booking: generates and returns the plaintext OTP for the passenger
    pub fn accept(&mut self) -> Result<String> {
        if self.status != BookingStatus::Requested {
            return Err(CoreError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "Accepted".into(),
            });
        }
        let otp = generate_otp();
        self.pickup_otp_hash = Some(hash_otp(&otp));
        self.status = BookingStatus::Accepted;
        Ok(otp)
    }

    /// Driver verifies passenger's OTP at physical pickup (enforces max 3 failed attempts lockout)
    pub fn verify_pickup(&mut self, otp_input: &str) -> Result<()> {
        if self.status != BookingStatus::Accepted {
            return Err(CoreError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "PickupConfirmed".into(),
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
                self.status = BookingStatus::Disputed;
                return Err(CoreError::OtpAttemptsExceeded);
            }
            return Err(e);
        }
        self.status = BookingStatus::PickupConfirmed;
        self.failed_pickup_attempts = 0;
        self.confirmed_pickup_at = Some(Utc::now());
        Ok(())
    }

    /// Complete booking upon reaching destination
    pub fn complete(&mut self) -> Result<()> {
        if self.status != BookingStatus::PickupConfirmed {
            return Err(CoreError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "Completed".into(),
            });
        }
        self.status = BookingStatus::Completed;
        self.completed_at = Some(Utc::now());
        Ok(())
    }

    /// Cancel booking
    pub fn cancel(&mut self) -> Result<()> {
        if self.status == BookingStatus::Completed {
            return Err(CoreError::InvalidStateTransition {
                from: "Completed".into(),
                to: "Cancelled".into(),
            });
        }
        self.status = BookingStatus::Cancelled;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_booking_lifecycle_with_otp() {
        let mut booking = RideBooking::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Gender::Female,
            1,
            "Campus South Gate".into(),
            GeoPoint::new(12.9716, 77.5946),
            "City Railway Station".into(),
            GeoPoint::new(12.9780, 77.5696),
            8000,
        );
        assert_eq!(booking.status, BookingStatus::Requested);

        // Gender compatibility validation
        assert!(booking
            .validate_gender_compatibility(&Gender::Female)
            .is_ok());
        assert!(booking
            .validate_gender_compatibility(&Gender::Male)
            .is_err());

        // Accept generates OTP
        let otp = booking.accept().expect("Should accept");
        assert_eq!(booking.status, BookingStatus::Accepted);
        assert_eq!(otp.len(), 6);

        // Verification fails with wrong OTP
        assert!(booking.verify_pickup("123456").is_err());

        // Verification succeeds with correct OTP
        assert!(booking.verify_pickup(&otp).is_ok());
        assert_eq!(booking.status, BookingStatus::PickupConfirmed);

        // Complete
        assert!(booking.complete().is_ok());
        assert_eq!(booking.status, BookingStatus::Completed);
    }
}
