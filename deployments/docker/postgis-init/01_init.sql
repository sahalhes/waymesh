-- =====================================================================
-- Waymesh Node: Initial PostgreSQL + PostGIS Schema and Seed Data
-- =====================================================================

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "postgis";

-- 1. Identity & Nodes
CREATE TYPE identity_role AS ENUM ('user', 'driver', 'carrier', 'node_operator');

CREATE TABLE identities (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    public_key_ed25519 VARCHAR(64) NOT NULL UNIQUE,
    display_name VARCHAR(100) NOT NULL,
    gender VARCHAR(20) NOT NULL DEFAULT 'other',
    phone_hash VARCHAR(64),
    reputation_score DOUBLE PRECISION DEFAULT 5.00,
    total_trips_completed INTEGER DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 2. Journeys (Carriers & Commuters)
CREATE TYPE journey_status AS ENUM ('draft', 'active', 'in_progress', 'completed', 'cancelled');

CREATE TABLE journeys (
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
    suggested_cost_per_seat INTEGER DEFAULT 8000, -- minor units (8000 = ₹80.00)
    status journey_status NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_journeys_origin_point ON journeys USING GIST(origin_point);
CREATE INDEX idx_journeys_destination_point ON journeys USING GIST(destination_point);
CREATE INDEX idx_journeys_corridor ON journeys USING GIST(route_corridor);
CREATE INDEX idx_journeys_dep_window ON journeys (departure_time_window_start, departure_time_window_end);
CREATE INDEX idx_journeys_status ON journeys (status);

-- 3. Ride Bookings (Passengers)
CREATE TYPE booking_status AS ENUM ('requested', 'accepted', 'pickup_confirmed', 'completed', 'cancelled', 'disputed');

CREATE TABLE ride_bookings (
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
    status booking_status NOT NULL DEFAULT 'requested',
    confirmed_pickup_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_ride_bookings_journey ON ride_bookings(journey_id);
CREATE INDEX idx_ride_bookings_passenger ON ride_bookings(passenger_id);

-- 4. Parcels (Logistics & Crowdshipping)
CREATE TYPE parcel_status AS ENUM (
    'draft', 'published', 'matched', 'pickup_pending', 
    'in_transit', 'delivered', 'returned', 'disputed', 'cancelled'
);

CREATE TABLE parcels (
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
    status parcel_status NOT NULL DEFAULT 'published',
    picked_up_at TIMESTAMPTZ,
    delivered_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_parcels_pickup_point ON parcels USING GIST(pickup_point);
CREATE INDEX idx_parcels_dropoff_point ON parcels USING GIST(dropoff_point);
CREATE INDEX idx_parcels_deadline ON parcels(deadline_time);
CREATE INDEX idx_parcels_status ON parcels(status);

-- =====================================================================
-- 5. Open Passenger Ride Requests (Demand-Side Publishing)
-- =====================================================================
CREATE TABLE IF NOT EXISTS ride_requests (
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
);

CREATE INDEX IF NOT EXISTS idx_ride_requests_origin ON ride_requests USING GIST(origin_point);
CREATE INDEX IF NOT EXISTS idx_ride_requests_dest ON ride_requests USING GIST(destination_point);
CREATE INDEX IF NOT EXISTS idx_ride_requests_status ON ride_requests(status);

-- =====================================================================
-- Seed Data: Campus ↔ Railway Station Corridor
-- =====================================================================

-- Identity 1: Commuter Driver (Rajesh - Graduate Student, Male)
INSERT INTO identities (id, public_key_ed25519, display_name, gender, reputation_score, total_trips_completed)
VALUES (
    '11111111-1111-1111-1111-111111111111',
    '3b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29',
    'Rajesh Kumar (Campus Commuter)',
    'male',
    4.95,
    34
);

-- Identity 2: Passenger (Priya - First Year Student, Female)
INSERT INTO identities (id, public_key_ed25519, display_name, gender, reputation_score, total_trips_completed)
VALUES (
    '22222222-2222-2222-2222-222222222222',
    'a591a6d40bf420404a011733cfb7b190d62c65bf0bcda32b57b277d9ad9f146e',
    'Priya Nair (Student)',
    'female',
    5.00,
    12
);

-- Identity 3: Commuter Driver (Sunita - Research Scholar, Female)
INSERT INTO identities (id, public_key_ed25519, display_name, gender, reputation_score, total_trips_completed)
VALUES (
    '33333333-3333-3333-3333-333333333333',
    '4c7b38cddeb7b53e73b4b9e13b7f1e84764326882ef354b74bd159b29c60eb30',
    'Sunita Rao (Campus Commuter)',
    'female',
    4.98,
    42
);

-- Identity 4: Passenger (Rohan - Engineering Student, Male)
INSERT INTO identities (id, public_key_ed25519, display_name, gender, reputation_score, total_trips_completed)
VALUES (
    '44444444-4444-4444-4444-444444444444',
    '5d8c49deefc8c64f84c5caf24c8f2f95875437993fa465c85ce260c30d71fc41',
    'Rohan Verma (Student)',
    'male',
    4.90,
    8
);

-- Active Corridor Journey 1: Campus North Gate -> Central Railway Station (Rajesh, Male)
INSERT INTO journeys (
    id,
    driver_id,
    driver_gender,
    origin_name,
    origin_point,
    destination_name,
    destination_point,
    route_corridor,
    departure_time_window_start,
    departure_time_window_end,
    available_seats,
    cargo_weight_capacity_kg,
    cargo_volume_liters,
    flexible_detour_minutes,
    suggested_cost_per_seat,
    status
) VALUES (
    'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa',
    '11111111-1111-1111-1111-111111111111',
    'male',
    'Campus North Gate',
    ST_SetSRID(ST_MakePoint(77.5946, 12.9716), 4326),
    'City Central Railway Station',
    ST_SetSRID(ST_MakePoint(77.5696, 12.9780), 4326),
    ST_SetSRID(ST_MakeLine(ST_MakePoint(77.5946, 12.9716), ST_MakePoint(77.5696, 12.9780)), 4326),
    NOW() + INTERVAL '1 hour',
    NOW() + INTERVAL '1 hour 30 minutes',
    3,
    15.00,
    25.00,
    15,
    8000,
    'active'
);

-- Active Corridor Journey 2: Campus North Gate -> Central Railway Station (Sunita, Female)
INSERT INTO journeys (
    id,
    driver_id,
    driver_gender,
    origin_name,
    origin_point,
    destination_name,
    destination_point,
    route_corridor,
    departure_time_window_start,
    departure_time_window_end,
    available_seats,
    cargo_weight_capacity_kg,
    cargo_volume_liters,
    flexible_detour_minutes,
    suggested_cost_per_seat,
    status
) VALUES (
    'bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb',
    '33333333-3333-3333-3333-333333333333',
    'female',
    'Campus North Gate',
    ST_SetSRID(ST_MakePoint(77.5946, 12.9716), 4326),
    'City Central Railway Station',
    ST_SetSRID(ST_MakePoint(77.5696, 12.9780), 4326),
    ST_SetSRID(ST_MakeLine(ST_MakePoint(77.5946, 12.9716), ST_MakePoint(77.5696, 12.9780)), 4326),
    NOW() + INTERVAL '1 hour',
    NOW() + INTERVAL '1 hour 30 minutes',
    3,
    15.00,
    25.00,
    15,
    8000,
    'active'
);
    'active'
);
