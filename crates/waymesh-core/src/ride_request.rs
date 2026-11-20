use crate::error::{CoreError, Result};
use crate::identity::Gender;
use crate::journey::GeoPoint;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RideRequestStatus {
    Published,
    Matched,
    Cancelled,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RideRequest {
    pub id: Uuid,
    pub passenger_id: Uuid,
    pub passenger_gender: Gender,
    pub origin_name: String,
    pub origin_point: GeoPoint,
    pub destination_name: String,
    pub destination_point: GeoPoint,
    pub desired_time: DateTime<Utc>,
    pub time_window_minutes: u32,
    pub seats: u32,
    pub max_cost_share: Option<u32>, // minor units (e.g. 8000 = ₹80.00)
    pub status: RideRequestStatus,
    pub assigned_journey_id: Option<Uuid>,
    pub assigned_booking_id: Option<Uuid>,
    pub pickup_otp: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl RideRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        passenger_id: Uuid,
        passenger_gender: Gender,
        origin_name: String,
        origin_point: GeoPoint,
        destination_name: String,
        destination_point: GeoPoint,
        desired_time: DateTime<Utc>,
        seats: u32,
        max_cost_share: Option<u32>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            passenger_id,
            passenger_gender,
            origin_name,
            origin_point,
            destination_name,
            destination_point,
            desired_time,
            time_window_minutes: 30,
            seats: seats.max(1),
            max_cost_share,
            status: RideRequestStatus::Published,
            assigned_journey_id: None,
            assigned_booking_id: None,
            pickup_otp: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn match_with_journey(
        &mut self,
        journey_id: Uuid,
        booking_id: Uuid,
        otp: Option<String>,
    ) -> Result<()> {
        if self.status != RideRequestStatus::Published {
            return Err(CoreError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "Matched".into(),
            });
        }
        self.status = RideRequestStatus::Matched;
        self.assigned_journey_id = Some(journey_id);
        self.assigned_booking_id = Some(booking_id);
        self.pickup_otp = otp;
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn cancel(&mut self) -> Result<()> {
        if self.status == RideRequestStatus::Matched {
            return Err(CoreError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "Cancelled".into(),
            });
        }
        self.status = RideRequestStatus::Cancelled;
        self.updated_at = Utc::now();
        Ok(())
    }
}
