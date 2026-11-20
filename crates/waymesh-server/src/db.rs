use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::str::FromStr;
use std::sync::Arc;
use tokio_postgres::{Client, NoTls};
use uuid::Uuid;
use waymesh_core::{
    BookingStatus, Gender, GeoPoint, Identity, Journey, JourneyStatus, Parcel, ParcelStatus,
    RideBooking, RideRequest, RideRequestStatus,
};

pub struct DbClient {
    client: Arc<Client>,
}

impl DbClient {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let (client, connection) = tokio_postgres::connect(database_url, NoTls)
            .await
            .context("Failed to connect to PostgreSQL")?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::error!("PostgreSQL connection error: {}", e);
            }
        });

        let client = Arc::new(client);

        // Ensure extensions exist
        client
            .execute("CREATE EXTENSION IF NOT EXISTS \"uuid-ossp\";", &[])
            .await
            .ok();
        client
            .execute("CREATE EXTENSION IF NOT EXISTS \"postgis\";", &[])
            .await
            .ok();

        // Auto-initialize tables if not present (Zero-config for fresh Supabase/Postgres)
        client
            .execute(
                "CREATE TABLE IF NOT EXISTS identities (
                    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                    public_key_ed25519 VARCHAR(64) NOT NULL UNIQUE,
                    display_name VARCHAR(100) NOT NULL,
                    gender VARCHAR(20) NOT NULL DEFAULT 'other',
                    email VARCHAR(255),
                    oauth_provider VARCHAR(50),
                    oauth_sub VARCHAR(100),
                    is_verified BOOLEAN DEFAULT FALSE,
                    phone_hash VARCHAR(64),
                    reputation_score DOUBLE PRECISION DEFAULT 5.00,
                    total_trips_completed INTEGER DEFAULT 0,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );",
                &[],
            )
            .await
            .ok();

        // Idempotent column migrations for OAuth & verification
        let _ = client
            .execute(
                "ALTER TABLE identities ADD COLUMN IF NOT EXISTS email VARCHAR(255);",
                &[],
            )
            .await;
        let _ = client
            .execute(
                "ALTER TABLE identities ADD COLUMN IF NOT EXISTS oauth_provider VARCHAR(50);",
                &[],
            )
            .await;
        let _ = client
            .execute(
                "ALTER TABLE identities ADD COLUMN IF NOT EXISTS oauth_sub VARCHAR(100);",
                &[],
            )
            .await;
        let _ = client.execute("ALTER TABLE identities ADD COLUMN IF NOT EXISTS is_verified BOOLEAN DEFAULT FALSE;", &[]).await;

        client
            .execute(
                "CREATE TABLE IF NOT EXISTS journeys (
                    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                    driver_id UUID NOT NULL REFERENCES identities(id) ON DELETE RESTRICT,
                    driver_gender VARCHAR(20) NOT NULL DEFAULT 'other',
                    origin_name VARCHAR(255) NOT NULL,
                    origin_point GEOMETRY(Point, 4326) NOT NULL,
                    destination_name VARCHAR(255) NOT NULL,
                    destination_point GEOMETRY(Point, 4326) NOT NULL,
                    route_corridor GEOMETRY(LineString, 4326),
                    corridor_buffer_meters INTEGER DEFAULT 1500,
                    departure_time_window_start TIMESTAMPTZ NOT NULL,
                    departure_time_window_end TIMESTAMPTZ NOT NULL,
                    available_seats INTEGER NOT NULL DEFAULT 1,
                    cargo_weight_capacity_kg DOUBLE PRECISION DEFAULT 0.00,
                    cargo_volume_liters DOUBLE PRECISION DEFAULT 0.00,
                    flexible_detour_minutes INTEGER DEFAULT 15,
                    suggested_cost_share_currency VARCHAR(3) DEFAULT 'INR',
                    suggested_cost_per_seat INTEGER DEFAULT 8000,
                    status VARCHAR(32) NOT NULL DEFAULT 'active',
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );",
                &[],
            )
            .await
            .ok();

        client
            .execute(
                "CREATE TABLE IF NOT EXISTS ride_bookings (
                    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                    journey_id UUID NOT NULL REFERENCES journeys(id) ON DELETE CASCADE,
                    passenger_id UUID NOT NULL REFERENCES identities(id) ON DELETE RESTRICT,
                    passenger_gender VARCHAR(20) NOT NULL DEFAULT 'other',
                    seats_booked INTEGER NOT NULL DEFAULT 1,
                    pickup_point GEOMETRY(Point, 4326) NOT NULL,
                    dropoff_point GEOMETRY(Point, 4326) NOT NULL,
                    pickup_name VARCHAR(255) NOT NULL,
                    dropoff_name VARCHAR(255) NOT NULL,
                    agreed_cost_share INTEGER NOT NULL,
                    pickup_otp_hash VARCHAR(64),
                    status VARCHAR(32) NOT NULL DEFAULT 'requested',
                    confirmed_pickup_at TIMESTAMPTZ,
                    completed_at TIMESTAMPTZ,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );",
                &[],
            )
            .await
            .ok();

        client
            .execute(
                "CREATE TABLE IF NOT EXISTS parcels (
                    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                    sender_id UUID NOT NULL REFERENCES identities(id) ON DELETE RESTRICT,
                    recipient_name VARCHAR(100) NOT NULL,
                    recipient_phone_hash VARCHAR(64) NOT NULL,
                    pickup_point GEOMETRY(Point, 4326) NOT NULL,
                    dropoff_point GEOMETRY(Point, 4326) NOT NULL,
                    pickup_address VARCHAR(255) NOT NULL,
                    dropoff_address VARCHAR(255) NOT NULL,
                    weight_kg DOUBLE PRECISION NOT NULL,
                    volume_liters DOUBLE PRECISION NOT NULL,
                    contents_declaration TEXT NOT NULL,
                    prohibited_items_screening_acknowledged BOOLEAN NOT NULL DEFAULT TRUE,
                    deadline_time TIMESTAMPTZ NOT NULL,
                    offered_compensation INTEGER NOT NULL,
                    assigned_journey_id UUID REFERENCES journeys(id) ON DELETE SET NULL,
                    carrier_id UUID REFERENCES identities(id) ON DELETE SET NULL,
                    pickup_otp_hash VARCHAR(64),
                    delivery_otp_hash VARCHAR(64),
                    status VARCHAR(32) NOT NULL DEFAULT 'published',
                    picked_up_at TIMESTAMPTZ,
                    delivered_at TIMESTAMPTZ,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );",
                &[],
            )
            .await
            .ok();

        client
            .execute(
                "CREATE TABLE IF NOT EXISTS ride_requests (
                    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
                    passenger_id UUID NOT NULL REFERENCES identities(id) ON DELETE RESTRICT,
                    passenger_gender VARCHAR(20) NOT NULL DEFAULT 'other',
                    origin_name VARCHAR(255) NOT NULL,
                    origin_point GEOMETRY(Point, 4326) NOT NULL,
                    destination_name VARCHAR(255) NOT NULL,
                    destination_point GEOMETRY(Point, 4326) NOT NULL,
                    desired_time TIMESTAMPTZ NOT NULL,
                    time_window_minutes INTEGER DEFAULT 30,
                    seats INTEGER NOT NULL DEFAULT 1,
                    max_cost_share INTEGER,
                    status VARCHAR(32) NOT NULL DEFAULT 'published',
                    assigned_journey_id UUID REFERENCES journeys(id) ON DELETE SET NULL,
                    assigned_booking_id UUID REFERENCES ride_bookings(id) ON DELETE SET NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );",
                &[],
            )
            .await
            .ok();

        // Spatial and search indexes
        client.execute("CREATE INDEX IF NOT EXISTS idx_journeys_origin ON journeys USING GIST(origin_point);", &[]).await.ok();
        client.execute("CREATE INDEX IF NOT EXISTS idx_journeys_dest ON journeys USING GIST(destination_point);", &[]).await.ok();
        client.execute("CREATE INDEX IF NOT EXISTS idx_parcels_pickup ON parcels USING GIST(pickup_point);", &[]).await.ok();
        client.execute("CREATE INDEX IF NOT EXISTS idx_parcels_dropoff ON parcels USING GIST(dropoff_point);", &[]).await.ok();
        client.execute("CREATE INDEX IF NOT EXISTS idx_ride_requests_origin ON ride_requests USING GIST(origin_point);", &[]).await.ok();
        client.execute("CREATE INDEX IF NOT EXISTS idx_ride_requests_dest ON ride_requests USING GIST(destination_point);", &[]).await.ok();

        // Ensure gender columns exist for safe migrations on existing tables
        client
            .execute(
                "ALTER TABLE identities ADD COLUMN IF NOT EXISTS gender VARCHAR(20) NOT NULL DEFAULT 'other';",
                &[],
            )
            .await
            .ok();
        client
            .execute(
                "ALTER TABLE journeys ADD COLUMN IF NOT EXISTS driver_gender VARCHAR(20) NOT NULL DEFAULT 'other';",
                &[],
            )
            .await
            .ok();
        client
            .execute(
                "ALTER TABLE ride_bookings ADD COLUMN IF NOT EXISTS passenger_gender VARCHAR(20) NOT NULL DEFAULT 'other';",
                &[],
            )
            .await
            .ok();

        Ok(Self { client })
    }

    pub async fn load_identities(&self) -> Result<Vec<Identity>> {
        let rows = self
            .client
            .query(
                "SELECT id, public_key_ed25519, display_name, COALESCE(gender, 'other')::text, phone_hash, 
                        reputation_score::float4, total_trips_completed::int4, email, oauth_provider, oauth_sub,
                        COALESCE(is_verified, false), created_at, updated_at 
                 FROM identities;",
                &[],
            )
            .await?;

        let mut list = Vec::new();
        for r in rows {
            let id: Uuid = r.get(0);
            let public_key_ed25519: String = r.get(1);
            let display_name: String = r.get(2);
            let gender_str: String = r.get(3);
            let phone_hash: Option<String> = r.get(4);
            let reputation_score: f32 = r.get(5);
            let total_trips_completed: i32 = r.get(6);
            let email: Option<String> = r.get(7);
            let oauth_provider: Option<String> = r.get(8);
            let oauth_sub: Option<String> = r.get(9);
            let is_verified: bool = r.get(10);
            let created_at: DateTime<Utc> = r.get(11);
            let updated_at: DateTime<Utc> = r.get(12);

            let gender = Gender::from_str(&gender_str).unwrap_or(Gender::Other);

            list.push(Identity {
                id,
                public_key_ed25519,
                display_name,
                gender,
                email,
                oauth_provider,
                oauth_sub,
                is_verified,
                phone_hash,
                reputation_score,
                total_trips_completed: total_trips_completed.max(0) as u32,
                created_at,
                updated_at,
            });
        }
        Ok(list)
    }

    pub async fn save_identity(&self, identity: &Identity) -> Result<()> {
        let gender_str = identity.gender.to_string();
        self.client
            .execute(
                "INSERT INTO identities (id, public_key_ed25519, display_name, gender, email, oauth_provider, oauth_sub, is_verified, phone_hash, reputation_score, total_trips_completed, created_at, updated_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10::float8::numeric, $11, $12, $13)
                 ON CONFLICT (id) DO UPDATE 
                 SET display_name = EXCLUDED.display_name,
                     gender = EXCLUDED.gender,
                     email = EXCLUDED.email,
                     oauth_provider = EXCLUDED.oauth_provider,
                     oauth_sub = EXCLUDED.oauth_sub,
                     is_verified = EXCLUDED.is_verified,
                     reputation_score = EXCLUDED.reputation_score,
                     total_trips_completed = EXCLUDED.total_trips_completed,
                     updated_at = EXCLUDED.updated_at;",
                &[
                    &identity.id,
                    &identity.public_key_ed25519,
                    &identity.display_name,
                    &gender_str,
                    &identity.email,
                    &identity.oauth_provider,
                    &identity.oauth_sub,
                    &identity.is_verified,
                    &identity.phone_hash,
                    &(identity.reputation_score as f64),
                    &(identity.total_trips_completed as i32),
                    &identity.created_at,
                    &identity.updated_at,
                ],
            )
            .await?;
        Ok(())
    }

    pub async fn load_journeys(&self) -> Result<Vec<Journey>> {
        let rows = self
            .client
            .query(
                "SELECT id, driver_id, COALESCE(driver_gender, 'other')::text, origin_name, 
                        ST_Y(origin_point)::float8, ST_X(origin_point)::float8,
                        destination_name,
                        ST_Y(destination_point)::float8, ST_X(destination_point)::float8,
                        corridor_buffer_meters::int4, departure_time_window_start, departure_time_window_end,
                        available_seats::int4, cargo_weight_capacity_kg::float8, cargo_volume_liters::float8,
                        flexible_detour_minutes::int4, suggested_cost_share_currency, suggested_cost_per_seat::int4,
                        status::text, created_at, updated_at
                 FROM journeys;",
                &[],
            )
            .await?;

        let mut list = Vec::new();
        for r in rows {
            let id: Uuid = r.get(0);
            let driver_id: Uuid = r.get(1);
            let driver_gender_str: String = r.get(2);
            let origin_name: String = r.get(3);
            let o_lat: f64 = r.get(4);
            let o_lng: f64 = r.get(5);
            let destination_name: String = r.get(6);
            let d_lat: f64 = r.get(7);
            let d_lng: f64 = r.get(8);
            let corridor_buffer_meters: i32 = r.get(9);
            let departure_start: DateTime<Utc> = r.get(10);
            let departure_end: DateTime<Utc> = r.get(11);
            let seats: i32 = r.get(12);
            let cargo_kg: f64 = r.get(13);
            let cargo_vol: f64 = r.get(14);
            let detour_mins: i32 = r.get(15);
            let currency: String = r.get(16);
            let cost_per_seat: i32 = r.get(17);
            let status_str: String = r.get(18);
            let created_at: DateTime<Utc> = r.get(19);
            let updated_at: DateTime<Utc> = r.get(20);

            let driver_gender = Gender::from_str(&driver_gender_str).unwrap_or(Gender::Other);
            let status = match status_str.to_lowercase().as_str() {
                "draft" => JourneyStatus::Draft,
                "in_progress" => JourneyStatus::InProgress,
                "completed" => JourneyStatus::Completed,
                "cancelled" => JourneyStatus::Cancelled,
                _ => JourneyStatus::Active,
            };

            list.push(Journey {
                id,
                driver_id,
                driver_gender,
                origin_name,
                origin_point: GeoPoint::new(o_lat, o_lng),
                destination_name,
                destination_point: GeoPoint::new(d_lat, d_lng),
                corridor_buffer_meters: corridor_buffer_meters.max(0) as u32,
                departure_time_window_start: departure_start,
                departure_time_window_end: departure_end,
                available_seats: seats.max(0) as u32,
                cargo_weight_capacity_kg: cargo_kg,
                cargo_volume_liters: cargo_vol,
                flexible_detour_minutes: detour_mins.max(0) as u32,
                suggested_cost_share_currency: currency,
                suggested_cost_per_seat: cost_per_seat.max(0) as u32,
                status,
                created_at,
                updated_at,
            });
        }
        Ok(list)
    }

    pub async fn save_journey(&self, journey: &Journey) -> Result<()> {
        let status_str = match journey.status {
            JourneyStatus::Draft => "draft",
            JourneyStatus::Active => "active",
            JourneyStatus::InProgress => "in_progress",
            JourneyStatus::Completed => "completed",
            JourneyStatus::Cancelled => "cancelled",
        };
        let driver_gender_str = journey.driver_gender.to_string();

        self.client
            .execute(
                "INSERT INTO journeys (
                    id, driver_id, driver_gender, origin_name, origin_point, destination_name, destination_point,
                    corridor_buffer_meters, departure_time_window_start, departure_time_window_end,
                    available_seats, cargo_weight_capacity_kg, cargo_volume_liters,
                    flexible_detour_minutes, suggested_cost_share_currency, suggested_cost_per_seat,
                    status, created_at, updated_at
                 ) VALUES (
                    $1, $2, $3, $4, ST_SetSRID(ST_MakePoint($6::float8, $5::float8), 4326), $7, ST_SetSRID(ST_MakePoint($9::float8, $8::float8), 4326),
                    $10, $11, $12, $13, $14::float8, $15::float8, $16, $17, $18, $19, $20, $21
                 )
                 ON CONFLICT (id) DO UPDATE
                 SET available_seats = EXCLUDED.available_seats,
                     driver_gender = EXCLUDED.driver_gender,
                     status = EXCLUDED.status,
                     updated_at = EXCLUDED.updated_at;",
                &[
                    &journey.id,
                    &journey.driver_id,
                    &driver_gender_str,
                    &journey.origin_name,
                    &journey.origin_point.latitude,
                    &journey.origin_point.longitude,
                    &journey.destination_name,
                    &journey.destination_point.latitude,
                    &journey.destination_point.longitude,
                    &(journey.corridor_buffer_meters as i32),
                    &journey.departure_time_window_start,
                    &journey.departure_time_window_end,
                    &(journey.available_seats as i32),
                    &journey.cargo_weight_capacity_kg,
                    &journey.cargo_volume_liters,
                    &(journey.flexible_detour_minutes as i32),
                    &journey.suggested_cost_share_currency,
                    &(journey.suggested_cost_per_seat as i32),
                    &status_str,
                    &journey.created_at,
                    &journey.updated_at,
                ],
            )
            .await?;
        Ok(())
    }

    pub async fn update_journey_status(&self, id: Uuid, status: JourneyStatus) -> Result<()> {
        let status_str = match status {
            JourneyStatus::Draft => "draft",
            JourneyStatus::Active => "active",
            JourneyStatus::InProgress => "in_progress",
            JourneyStatus::Completed => "completed",
            JourneyStatus::Cancelled => "cancelled",
        };
        let now = Utc::now();
        self.client
            .execute(
                "UPDATE journeys SET status = ($1::text)::journey_status, updated_at = $2 WHERE id = $3;",
                &[&status_str, &now, &id],
            )
            .await?;
        Ok(())
    }

    pub async fn load_bookings(&self) -> Result<Vec<RideBooking>> {
        let rows = self
            .client
            .query(
                "SELECT id, journey_id, passenger_id, COALESCE(passenger_gender, 'other')::text, seats_booked::int4,
                        pickup_name, ST_Y(pickup_point)::float8, ST_X(pickup_point)::float8,
                        dropoff_name, ST_Y(dropoff_point)::float8, ST_X(dropoff_point)::float8,
                        agreed_cost_share::int4, pickup_otp_hash, status::text,
                        confirmed_pickup_at, completed_at, created_at
                 FROM ride_bookings;",
                &[],
            )
            .await?;

        let mut list = Vec::new();
        for r in rows {
            let id: Uuid = r.get(0);
            let journey_id: Uuid = r.get(1);
            let passenger_id: Uuid = r.get(2);
            let passenger_gender_str: String = r.get(3);
            let seats_booked: i32 = r.get(4);
            let pickup_name: String = r.get(5);
            let p_lat: f64 = r.get(6);
            let p_lng: f64 = r.get(7);
            let dropoff_name: String = r.get(8);
            let d_lat: f64 = r.get(9);
            let d_lng: f64 = r.get(10);
            let agreed_cost_share: i32 = r.get(11);
            let pickup_otp_hash: Option<String> = r.get(12);
            let status_str: String = r.get(13);
            let confirmed_pickup_at: Option<DateTime<Utc>> = r.get(14);
            let completed_at: Option<DateTime<Utc>> = r.get(15);
            let created_at: DateTime<Utc> = r.get(16);

            let passenger_gender = Gender::from_str(&passenger_gender_str).unwrap_or(Gender::Other);
            let status = match status_str.to_lowercase().as_str() {
                "accepted" => BookingStatus::Accepted,
                "pickup_confirmed" => BookingStatus::PickupConfirmed,
                "completed" => BookingStatus::Completed,
                "cancelled" => BookingStatus::Cancelled,
                "disputed" => BookingStatus::Disputed,
                _ => BookingStatus::Requested,
            };

            list.push(RideBooking {
                id,
                journey_id,
                passenger_id,
                passenger_gender,
                seats_booked: seats_booked.max(0) as u32,
                pickup_name,
                pickup_point: GeoPoint::new(p_lat, p_lng),
                dropoff_name,
                dropoff_point: GeoPoint::new(d_lat, d_lng),
                agreed_cost_share: agreed_cost_share.max(0) as u32,
                pickup_otp_hash,
                failed_pickup_attempts: 0,
                status,
                confirmed_pickup_at,
                completed_at,
                created_at,
            });
        }
        Ok(list)
    }

    pub async fn save_booking(&self, booking: &RideBooking) -> Result<()> {
        let status_str = match booking.status {
            BookingStatus::Requested => "requested",
            BookingStatus::Accepted => "accepted",
            BookingStatus::PickupConfirmed => "pickup_confirmed",
            BookingStatus::Completed => "completed",
            BookingStatus::Cancelled => "cancelled",
            BookingStatus::Disputed => "disputed",
        };
        let passenger_gender_str = booking.passenger_gender.to_string();

        self.client
            .execute(
                "INSERT INTO ride_bookings (
                    id, journey_id, passenger_id, passenger_gender, seats_booked,
                    pickup_point, dropoff_point, pickup_name, dropoff_name,
                    agreed_cost_share, pickup_otp_hash, status, confirmed_pickup_at,
                    completed_at, created_at
                 ) VALUES (
                    $1, $2, $3, $4, $5,
                    ST_SetSRID(ST_MakePoint($7::float8, $6::float8), 4326), ST_SetSRID(ST_MakePoint($9::float8, $8::float8), 4326),
                    $10, $11, $12, $13, $14, $15, $16, $17
                 )
                 ON CONFLICT (id) DO UPDATE
                 SET status = EXCLUDED.status,
                     passenger_gender = EXCLUDED.passenger_gender,
                     pickup_otp_hash = EXCLUDED.pickup_otp_hash,
                     confirmed_pickup_at = EXCLUDED.confirmed_pickup_at,
                     completed_at = EXCLUDED.completed_at;",
                &[
                    &booking.id,
                    &booking.journey_id,
                    &booking.passenger_id,
                    &passenger_gender_str,
                    &(booking.seats_booked as i32),
                    &booking.pickup_point.latitude,
                    &booking.pickup_point.longitude,
                    &booking.dropoff_point.latitude,
                    &booking.dropoff_point.longitude,
                    &booking.pickup_name,
                    &booking.dropoff_name,
                    &(booking.agreed_cost_share as i32),
                    &booking.pickup_otp_hash,
                    &status_str,
                    &booking.confirmed_pickup_at,
                    &booking.completed_at,
                    &booking.created_at,
                ],
            )
            .await?;
        Ok(())
    }

    pub async fn load_parcels(&self) -> Result<Vec<Parcel>> {
        let rows = self
            .client
            .query(
                "SELECT id, sender_id, recipient_name, recipient_phone_hash,
                        ST_Y(pickup_point)::float8, ST_X(pickup_point)::float8,
                        ST_Y(dropoff_point)::float8, ST_X(dropoff_point)::float8,
                        pickup_address, dropoff_address, weight_kg::float8, volume_liters::float8,
                        contents_declaration, prohibited_items_screening_acknowledged,
                        deadline_time, offered_compensation::int4, assigned_journey_id,
                        carrier_id, pickup_otp_hash, delivery_otp_hash, status::text,
                        picked_up_at, delivered_at, created_at
                 FROM parcels;",
                &[],
            )
            .await?;

        let mut list = Vec::new();
        for r in rows {
            let id: Uuid = r.get(0);
            let sender_id: Uuid = r.get(1);
            let recipient_name: String = r.get(2);
            let recipient_phone_hash: String = r.get(3);
            let p_lat: f64 = r.get(4);
            let p_lng: f64 = r.get(5);
            let d_lat: f64 = r.get(6);
            let d_lng: f64 = r.get(7);
            let pickup_address: String = r.get(8);
            let dropoff_address: String = r.get(9);
            let weight_kg: f64 = r.get(10);
            let volume_liters: f64 = r.get(11);
            let contents_declaration: String = r.get(12);
            let prohibited_screened: bool = r.get(13);
            let deadline_time: DateTime<Utc> = r.get(14);
            let offered_compensation: i32 = r.get(15);
            let assigned_journey_id: Option<Uuid> = r.get(16);
            let carrier_id: Option<Uuid> = r.get(17);
            let pickup_otp_hash: Option<String> = r.get(18);
            let delivery_otp_hash: Option<String> = r.get(19);
            let status_str: String = r.get(20);
            let picked_up_at: Option<DateTime<Utc>> = r.get(21);
            let delivered_at: Option<DateTime<Utc>> = r.get(22);
            let created_at: DateTime<Utc> = r.get(23);

            let status = match status_str.to_lowercase().as_str() {
                "draft" => ParcelStatus::Draft,
                "matched" => ParcelStatus::Matched,
                "pickup_pending" => ParcelStatus::PickupPending,
                "in_transit" => ParcelStatus::InTransit,
                "delivered" => ParcelStatus::Delivered,
                "returned" => ParcelStatus::Returned,
                "disputed" => ParcelStatus::Disputed,
                "cancelled" => ParcelStatus::Cancelled,
                _ => ParcelStatus::Published,
            };

            list.push(Parcel {
                id,
                sender_id,
                recipient_name,
                recipient_phone_hash,
                pickup_point: GeoPoint::new(p_lat, p_lng),
                dropoff_point: GeoPoint::new(d_lat, d_lng),
                pickup_address,
                dropoff_address,
                weight_kg,
                volume_liters,
                contents_declaration,
                prohibited_items_screened: prohibited_screened,
                deadline_time,
                offered_compensation: offered_compensation.max(0) as u32,
                assigned_journey_id,
                carrier_id,
                pickup_otp_hash,
                delivery_otp_hash,
                failed_pickup_attempts: 0,
                failed_delivery_attempts: 0,
                status,
                picked_up_at,
                delivered_at,
                created_at,
            });
        }
        Ok(list)
    }

    pub async fn save_parcel(&self, parcel: &Parcel) -> Result<()> {
        let status_str = match parcel.status {
            ParcelStatus::Draft => "draft",
            ParcelStatus::Published => "published",
            ParcelStatus::Matched => "matched",
            ParcelStatus::PickupPending => "pickup_pending",
            ParcelStatus::InTransit => "in_transit",
            ParcelStatus::Delivered => "delivered",
            ParcelStatus::Returned => "returned",
            ParcelStatus::Disputed => "disputed",
            ParcelStatus::Cancelled => "cancelled",
        };

        self.client
            .execute(
                "INSERT INTO parcels (
                    id, sender_id, recipient_name, recipient_phone_hash,
                    pickup_point, dropoff_point, pickup_address, dropoff_address,
                    weight_kg, volume_liters, contents_declaration, prohibited_items_screening_acknowledged,
                    deadline_time, offered_compensation, assigned_journey_id, carrier_id,
                    pickup_otp_hash, delivery_otp_hash, status, picked_up_at, delivered_at, created_at
                 ) VALUES (
                    $1, $2, $3, $4,
                    ST_SetSRID(ST_MakePoint($6::float8, $5::float8), 4326), ST_SetSRID(ST_MakePoint($8::float8, $7::float8), 4326),
                    $9, $10, $11::float8, $12::float8, $13, $14, $15, $16, $17, $18, $19, $20, $21,
                    $22, $23, $24
                 )
                 ON CONFLICT (id) DO UPDATE
                 SET assigned_journey_id = EXCLUDED.assigned_journey_id,
                     carrier_id = EXCLUDED.carrier_id,
                     pickup_otp_hash = EXCLUDED.pickup_otp_hash,
                     delivery_otp_hash = EXCLUDED.delivery_otp_hash,
                     status = EXCLUDED.status,
                     picked_up_at = EXCLUDED.picked_up_at,
                     delivered_at = EXCLUDED.delivered_at;",
                &[
                    &parcel.id,
                    &parcel.sender_id,
                    &parcel.recipient_name,
                    &parcel.recipient_phone_hash,
                    &parcel.pickup_point.latitude,
                    &parcel.pickup_point.longitude,
                    &parcel.dropoff_point.latitude,
                    &parcel.dropoff_point.longitude,
                    &parcel.pickup_address,
                    &parcel.dropoff_address,
                    &parcel.weight_kg,
                    &parcel.volume_liters,
                    &parcel.contents_declaration,
                    &parcel.prohibited_items_screened,
                    &parcel.deadline_time,
                    &(parcel.offered_compensation as i32),
                    &parcel.assigned_journey_id,
                    &parcel.carrier_id,
                    &parcel.pickup_otp_hash,
                    &parcel.delivery_otp_hash,
                    &status_str,
                    &parcel.picked_up_at,
                    &parcel.delivered_at,
                    &parcel.created_at,
                ],
            )
            .await?;
        Ok(())
    }

    pub async fn load_ride_requests(&self) -> Result<Vec<RideRequest>> {
        let rows = self
            .client
            .query(
                "SELECT id, passenger_id, COALESCE(passenger_gender, 'other')::text, origin_name,
                        ST_Y(origin_point)::float8, ST_X(origin_point)::float8,
                        destination_name,
                        ST_Y(destination_point)::float8, ST_X(destination_point)::float8,
                        desired_time, time_window_minutes::int4, seats::int4, max_cost_share::int4,
                        status::text, assigned_journey_id, assigned_booking_id, created_at, updated_at
                 FROM ride_requests;",
                &[],
            )
            .await?;

        let mut list = Vec::new();
        for r in rows {
            let id: Uuid = r.get(0);
            let passenger_id: Uuid = r.get(1);
            let gender_str: String = r.get(2);
            let origin_name: String = r.get(3);
            let o_lat: f64 = r.get(4);
            let o_lng: f64 = r.get(5);
            let destination_name: String = r.get(6);
            let d_lat: f64 = r.get(7);
            let d_lng: f64 = r.get(8);
            let desired_time: DateTime<Utc> = r.get(9);
            let time_window_minutes: i32 = r.get(10);
            let seats: i32 = r.get(11);
            let max_cost_share: Option<i32> = r.get(12);
            let status_str: String = r.get(13);
            let assigned_journey_id: Option<Uuid> = r.get(14);
            let assigned_booking_id: Option<Uuid> = r.get(15);
            let created_at: DateTime<Utc> = r.get(16);
            let updated_at: DateTime<Utc> = r.get(17);

            let status = match status_str.to_lowercase().as_str() {
                "matched" => RideRequestStatus::Matched,
                "cancelled" => RideRequestStatus::Cancelled,
                "expired" => RideRequestStatus::Expired,
                _ => RideRequestStatus::Published,
            };

            list.push(RideRequest {
                id,
                passenger_id,
                passenger_gender: Gender::from_str(&gender_str).unwrap_or(Gender::Other),
                origin_name,
                origin_point: GeoPoint::new(o_lat, o_lng),
                destination_name,
                destination_point: GeoPoint::new(d_lat, d_lng),
                desired_time,
                time_window_minutes: time_window_minutes as u32,
                seats: seats as u32,
                max_cost_share: max_cost_share.map(|c| c as u32),
                status,
                assigned_journey_id,
                assigned_booking_id,
                pickup_otp: None,
                created_at,
                updated_at,
            });
        }
        Ok(list)
    }

    pub async fn save_ride_request(&self, req: &RideRequest) -> Result<()> {
        let gender_str = match req.passenger_gender {
            Gender::Male => "male",
            Gender::Female => "female",
            Gender::Other => "other",
        };

        let status_str = match req.status {
            RideRequestStatus::Published => "published",
            RideRequestStatus::Matched => "matched",
            RideRequestStatus::Cancelled => "cancelled",
            RideRequestStatus::Expired => "expired",
        };

        let max_cost = req.max_cost_share.map(|c| c as i32);

        self.client
            .execute(
                "INSERT INTO ride_requests (
                    id, passenger_id, passenger_gender, origin_name,
                    origin_point, destination_name, destination_point,
                    desired_time, time_window_minutes, seats, max_cost_share,
                    status, assigned_journey_id, assigned_booking_id, created_at, updated_at
                 ) VALUES (
                    $1, $2, $3, $4,
                    ST_SetSRID(ST_MakePoint($6::float8, $5::float8), 4326),
                    $7,
                    ST_SetSRID(ST_MakePoint($9::float8, $8::float8), 4326),
                    $10, $11, $12, $13,
                    $14, $15, $16, $17, $18
                 )
                 ON CONFLICT (id) DO UPDATE
                 SET status = EXCLUDED.status,
                     assigned_journey_id = EXCLUDED.assigned_journey_id,
                     assigned_booking_id = EXCLUDED.assigned_booking_id,
                     updated_at = EXCLUDED.updated_at;",
                &[
                    &req.id,
                    &req.passenger_id,
                    &gender_str,
                    &req.origin_name,
                    &req.origin_point.latitude,
                    &req.origin_point.longitude,
                    &req.destination_name,
                    &req.destination_point.latitude,
                    &req.destination_point.longitude,
                    &req.desired_time,
                    &(req.time_window_minutes as i32),
                    &(req.seats as i32),
                    &max_cost,
                    &status_str,
                    &req.assigned_journey_id,
                    &req.assigned_booking_id,
                    &req.created_at,
                    &req.updated_at,
                ],
            )
            .await?;
        Ok(())
    }

    pub async fn update_ride_request_status(
        &self,
        id: Uuid,
        status: RideRequestStatus,
        journey_id: Option<Uuid>,
        booking_id: Option<Uuid>,
    ) -> Result<()> {
        let status_str = match status {
            RideRequestStatus::Published => "published",
            RideRequestStatus::Matched => "matched",
            RideRequestStatus::Cancelled => "cancelled",
            RideRequestStatus::Expired => "expired",
        };
        self.client
            .execute(
                "UPDATE ride_requests
                 SET status = $2, assigned_journey_id = $3, assigned_booking_id = $4, updated_at = NOW()
                 WHERE id = $1;",
                &[&id, &status_str, &journey_id, &booking_id],
            )
            .await?;
        Ok(())
    }
}
