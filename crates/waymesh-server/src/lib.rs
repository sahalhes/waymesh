use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::Infallible;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use uuid::Uuid;
use waymesh_core::{
    resolve_known_landmark, Gender, GeoPoint, Identity, Journey, JourneyStatus, Parcel,
    ParcelStatus, RideBooking, RideRequest, RideRequestStatus,
};
use waymesh_solver::{
    DemandItem, DemandType, JourneyCandidate, MatchProblem, MatchingSolver, Objective,
    SpatialCorridorHeuristic,
};

pub mod db;
pub use db::DbClient;

#[derive(Clone)]
pub struct AppState {
    pub identities: Arc<RwLock<HashMap<Uuid, Identity>>>,
    pub journeys: Arc<RwLock<HashMap<Uuid, Journey>>>,
    pub bookings: Arc<RwLock<HashMap<Uuid, RideBooking>>>,
    pub parcels: Arc<RwLock<HashMap<Uuid, Parcel>>>,
    pub ride_requests: Arc<RwLock<HashMap<Uuid, RideRequest>>>,
    pub sessions: Arc<RwLock<HashMap<String, Uuid>>>,
    pub solver: Arc<dyn MatchingSolver>,
    pub event_bus: broadcast::Sender<String>,
    pub db: Option<Arc<DbClient>>,
    pub server_url: String,
    pub campus_domain: String,
    pub http_client: reqwest::Client,
}

impl AppState {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(100);
        let server_url = std::env::var("SERVER_URL")
            .or_else(|_| std::env::var("WAYMESH_PUBLIC_URL"))
            .or_else(|_| std::env::var("WAYMESH_NODE_URL"))
            .unwrap_or_else(|_| "http://localhost:8080".into())
            .trim_end_matches('/')
            .to_string();

        let campus_domain = std::env::var("CAMPUS_DOMAIN")
            .unwrap_or_else(|_| "@cet.ac.in".into())
            .trim()
            .to_lowercase();

        Self {
            identities: Arc::new(RwLock::new(HashMap::new())),
            journeys: Arc::new(RwLock::new(HashMap::new())),
            bookings: Arc::new(RwLock::new(HashMap::new())),
            parcels: Arc::new(RwLock::new(HashMap::new())),
            ride_requests: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            solver: Arc::new(SpatialCorridorHeuristic::default()),
            event_bus: tx,
            db: None,
            server_url,
            campus_domain,
            http_client: reqwest::Client::new(),
        }
    }

    pub async fn from_database(database_url: &str) -> anyhow::Result<Self> {
        let db = DbClient::connect(database_url).await?;
        let loaded_identities = db.load_identities().await?;
        let loaded_journeys = db.load_journeys().await?;
        let loaded_bookings = db.load_bookings().await?;
        let loaded_parcels = db.load_parcels().await?;
        let loaded_ride_requests = db.load_ride_requests().await.unwrap_or_default();

        tracing::info!(
            "Hydrated state from PostgreSQL: {} identities, {} journeys, {} bookings, {} parcels, {} ride_requests",
            loaded_identities.len(),
            loaded_journeys.len(),
            loaded_bookings.len(),
            loaded_parcels.len(),
            loaded_ride_requests.len()
        );

        let mut id_map = HashMap::new();
        for id in loaded_identities {
            id_map.insert(id.id, id);
        }

        let mut j_map = HashMap::new();
        for j in loaded_journeys {
            j_map.insert(j.id, j);
        }

        let mut b_map = HashMap::new();
        for b in loaded_bookings {
            b_map.insert(b.id, b);
        }

        let mut p_map = HashMap::new();
        for p in loaded_parcels {
            p_map.insert(p.id, p);
        }

        let mut rr_map = HashMap::new();
        for rr in loaded_ride_requests {
            rr_map.insert(rr.id, rr);
        }

        let (tx, _rx) = broadcast::channel(100);

        let server_url = std::env::var("SERVER_URL")
            .or_else(|_| std::env::var("WAYMESH_PUBLIC_URL"))
            .or_else(|_| std::env::var("WAYMESH_NODE_URL"))
            .unwrap_or_else(|_| "http://localhost:8080".into())
            .trim_end_matches('/')
            .to_string();

        let campus_domain = std::env::var("CAMPUS_DOMAIN")
            .unwrap_or_else(|_| "@cet.ac.in".into())
            .trim()
            .to_lowercase();

        Ok(Self {
            identities: Arc::new(RwLock::new(id_map)),
            journeys: Arc::new(RwLock::new(j_map)),
            bookings: Arc::new(RwLock::new(b_map)),
            parcels: Arc::new(RwLock::new(p_map)),
            ride_requests: Arc::new(RwLock::new(rr_map)),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            solver: Arc::new(SpatialCorridorHeuristic::default()),
            event_bus: tx,
            db: Some(Arc::new(db)),
            server_url,
            campus_domain,
            http_client: reqwest::Client::new(),
        })
    }

    pub fn emit_event(&self, event_name: &str, payload: serde_json::Value) {
        let msg = serde_json::json!({
            "event": event_name,
            "timestamp": Utc::now(),
            "data": payload
        })
        .to_string();
        let _ = self.event_bus.send(msg);
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/mcp", post(handle_mcp_post).get(handle_mcp_get))
        .route("/.well-known/ai-plugin.json", get(serve_ai_plugin_manifest))
        .route("/openapi.yaml", get(serve_openapi_yaml))
        .route("/api/v1/identities", post(register_identity))
        .route("/api/v1/auth/oauth/authorize", get(oauth_authorize))
        .route("/api/v1/auth/oauth/callback", post(oauth_callback))
        .route("/api/v1/auth/oauth/token", post(oauth_token_exchange))
        .route("/api/v1/auth/me", get(get_current_authenticated_user))
        .route("/api/v1/journeys", post(publish_journey))
        .route("/api/v1/journeys/active", get(get_active_journeys))
        .route("/api/v1/journeys/:id/complete", post(complete_journey))
        .route("/api/v1/journeys/:id/cancel", post(cancel_journey))
        .route("/api/v1/matches/search", post(search_matches))
        .route("/api/v1/ride-requests", post(publish_ride_request))
        .route(
            "/api/v1/ride-requests/active",
            get(get_active_ride_requests),
        )
        .route("/api/v1/ride-requests/search", post(search_ride_requests))
        .route("/api/v1/ride-requests/:id", get(get_ride_request_by_id))
        .route(
            "/api/v1/ride-requests/:id/accept",
            post(accept_ride_request),
        )
        .route(
            "/api/v1/ride-requests/:id/cancel",
            post(cancel_ride_request),
        )
        .route("/api/v1/bookings", post(create_booking))
        .route("/api/v1/bookings/:id/accept", post(accept_booking))
        .route(
            "/api/v1/bookings/:id/verify-pickup",
            post(verify_booking_pickup),
        )
        .route("/api/v1/bookings/:id/complete", post(complete_booking))
        .route("/api/v1/bookings/:id/cancel", post(cancel_booking))
        .route("/api/v1/parcels", post(create_parcel))
        .route("/api/v1/parcels/:id/accept", post(accept_parcel))
        .route("/api/v1/parcels/:id/pickup", post(verify_parcel_pickup))
        .route("/api/v1/parcels/:id/deliver", post(verify_parcel_deliver))
        .route("/api/v1/parcels/:id/cancel", post(cancel_parcel))
        .route("/api/v1/events/stream", get(event_stream))
        .layer(axum::extract::DefaultBodyLimit::max(64 * 1024))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

// ----------------- Handlers -----------------

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "waymesh-node",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": Utc::now()
    }))
}

#[derive(Deserialize)]
pub struct RegisterIdentityRequest {
    pub public_key_ed25519: String,
    pub display_name: String,
    pub gender: Option<String>,
}

async fn register_identity(
    State(state): State<AppState>,
    Json(req): Json<RegisterIdentityRequest>,
) -> impl IntoResponse {
    let gender = req
        .gender
        .as_deref()
        .and_then(|g| Gender::from_str(g).ok())
        .unwrap_or(Gender::Other);
    let identity = Identity::new(req.public_key_ed25519, req.display_name, gender);
    let mut identities = state.identities.write().await;
    identities.insert(identity.id, identity.clone());
    if let Some(db) = &state.db {
        if let Err(e) = db.save_identity(&identity).await {
            tracing::error!("Failed to save identity to db: {}", e);
        }
    }
    (StatusCode::CREATED, Json(identity))
}

pub fn deserialize_flexible_datetime<'de, D>(
    deserializer: D,
) -> std::result::Result<DateTime<Utc>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let s = String::deserialize(deserializer)?;
    waymesh_core::parse_flexible_datetime(&s)
        .ok_or_else(|| D::Error::custom(format!("Invalid datetime string: '{}'", s)))
}

pub fn deserialize_flexible_datetime_opt<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<DateTime<Utc>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let opt = Option::<String>::deserialize(deserializer)?;
    match opt {
        Some(s) => {
            if s.trim().is_empty() {
                Ok(None)
            } else {
                waymesh_core::parse_flexible_datetime(&s)
                    .map(Some)
                    .ok_or_else(|| D::Error::custom(format!("Invalid datetime string: '{}'", s)))
            }
        }
        None => Ok(None),
    }
}

#[derive(Deserialize)]
pub struct PublishJourneyRequest {
    pub driver_id: Option<Uuid>,
    pub driver_name: Option<String>,
    pub driver_gender: Option<String>,
    #[serde(alias = "pickup_name")]
    pub origin_name: String,
    #[serde(alias = "pickup_lat", default)]
    pub origin_lat: f64,
    #[serde(alias = "pickup_lng", default)]
    pub origin_lng: f64,
    #[serde(alias = "dropoff_name")]
    pub destination_name: String,
    #[serde(alias = "dropoff_lat", default)]
    pub destination_lat: f64,
    #[serde(alias = "dropoff_lng", default)]
    pub destination_lng: f64,
    #[serde(deserialize_with = "deserialize_flexible_datetime")]
    pub departure_time: DateTime<Utc>,
    pub seats: Option<u32>,
    pub cargo_kg: Option<f64>,
    pub suggested_cost_per_seat: Option<u32>,
}

async fn publish_journey(
    State(state): State<AppState>,
    Json(req): Json<PublishJourneyRequest>,
) -> impl IntoResponse {
    let (driver_id, driver_gender) = if let Some(did) = req.driver_id {
        let gender = if let Some(g_str) = req.driver_gender.as_deref() {
            Gender::from_str(g_str).unwrap_or(Gender::Other)
        } else {
            let identities = state.identities.read().await;
            identities
                .get(&did)
                .map(|id| id.gender)
                .unwrap_or(Gender::Other)
        };
        (did, gender)
    } else {
        let gender = req
            .driver_gender
            .as_deref()
            .and_then(|g| Gender::from_str(g).ok())
            .unwrap_or(Gender::Other);
        let name = req
            .driver_name
            .unwrap_or_else(|| "Commuter Driver".to_string());
        let keypair = waymesh_core::crypto::KeyPair::generate();
        let new_identity = Identity::new(keypair.public_key_hex(), name, gender);
        let did = new_identity.id;
        {
            let mut identities = state.identities.write().await;
            identities.insert(did, new_identity.clone());
        }
        if let Some(db) = &state.db {
            let _ = db.save_identity(&new_identity).await;
        }
        (did, gender)
    };

    let origin_point = if req.origin_lat == 0.0 && req.origin_lng == 0.0 {
        resolve_known_landmark(&req.origin_name)
            .unwrap_or(GeoPoint::new(req.origin_lat, req.origin_lng))
    } else {
        GeoPoint::new(req.origin_lat, req.origin_lng)
    };
    let destination_point = if req.destination_lat == 0.0 && req.destination_lng == 0.0 {
        resolve_known_landmark(&req.destination_name)
            .unwrap_or(GeoPoint::new(req.destination_lat, req.destination_lng))
    } else {
        GeoPoint::new(req.destination_lat, req.destination_lng)
    };

    let mut journey = Journey::new(
        driver_id,
        driver_gender,
        req.origin_name,
        origin_point,
        req.destination_name,
        destination_point,
        req.departure_time,
        req.departure_time + chrono::Duration::minutes(30),
        req.seats.unwrap_or(3),
        req.cargo_kg.unwrap_or(10.0),
    );
    if let Some(cost) = req.suggested_cost_per_seat {
        journey.suggested_cost_per_seat = cost;
    }

    let mut journeys = state.journeys.write().await;
    journeys.insert(journey.id, journey.clone());

    if let Some(db) = &state.db {
        if let Err(e) = db.save_journey(&journey).await {
            tracing::error!("Failed to save journey to db: {}", e);
        }
    }

    state.emit_event(
        "journey.published",
        serde_json::json!({ "journey_id": journey.id, "driver_id": journey.driver_id }),
    );

    // Check for open passenger ride requests along this corridor
    let heuristic = SpatialCorridorHeuristic::default();
    let mut matching_requests = Vec::new();
    {
        let ride_requests = state.ride_requests.read().await;
        for r_req in ride_requests.values() {
            if r_req.status == RideRequestStatus::Published
                && r_req
                    .passenger_gender
                    .is_compatible_for_ride(&journey.driver_gender)
            {
                if let Some((detour, score)) = heuristic.score_corridor_fit(
                    &journey.origin_point,
                    &journey.destination_point,
                    journey.flexible_detour_minutes,
                    &r_req.origin_point,
                    &r_req.destination_point,
                ) {
                    matching_requests.push(serde_json::json!({
                        "request_id": r_req.id,
                        "passenger_id": r_req.passenger_id,
                        "passenger_gender": r_req.passenger_gender,
                        "origin_name": r_req.origin_name,
                        "destination_name": r_req.destination_name,
                        "desired_time": r_req.desired_time,
                        "seats": r_req.seats,
                        "detour_minutes": detour,
                        "match_score": score,
                    }));
                }
            }
        }
    }

    let mut journey_val = serde_json::to_value(&journey).unwrap_or_default();
    journey_val["matching_ride_requests"] = serde_json::json!(matching_requests);

    (StatusCode::CREATED, Json(journey_val))
}

async fn get_active_journeys(State(state): State<AppState>) -> impl IntoResponse {
    let journeys = state.journeys.read().await;
    let active: Vec<Journey> = journeys
        .values()
        .filter(|j| j.status == JourneyStatus::Active)
        .cloned()
        .collect();
    Json(active)
}

async fn complete_journey(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut journeys = state.journeys.write().await;
    let journey = journeys.get_mut(&id).ok_or(StatusCode::NOT_FOUND)?;
    journey.complete().map_err(|_| StatusCode::BAD_REQUEST)?;

    // Increment driver completed trips
    let mut identities = state.identities.write().await;
    if let Some(driver) = identities.get_mut(&journey.driver_id) {
        driver.increment_trips();
        if let Some(db) = &state.db {
            let _ = db.save_identity(driver).await;
        }
    }

    if let Some(db) = &state.db {
        let _ = db.update_journey_status(id, JourneyStatus::Completed).await;
    }

    state.emit_event("journey.completed", serde_json::json!({ "journey_id": id }));
    Ok(Json(journey.clone()))
}

async fn cancel_journey(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut journeys = state.journeys.write().await;
    let journey = journeys.get_mut(&id).ok_or(StatusCode::NOT_FOUND)?;
    journey.cancel().map_err(|_| StatusCode::BAD_REQUEST)?;

    if let Some(db) = &state.db {
        let _ = db.update_journey_status(id, JourneyStatus::Cancelled).await;
    }

    state.emit_event("journey.cancelled", serde_json::json!({ "journey_id": id }));

    // 1. Cascade cancellation to all non-completed bookings on this journey
    let mut bookings = state.bookings.write().await;
    for booking in bookings.values_mut() {
        if booking.journey_id == id
            && booking.status != waymesh_core::BookingStatus::Completed
            && booking.status != waymesh_core::BookingStatus::Cancelled
            && booking.cancel().is_ok()
        {
            if let Some(db) = &state.db {
                let _ = db.save_booking(booking).await;
            }
            state.emit_event(
                "booking.cancelled",
                serde_json::json!({
                    "booking_id": booking.id,
                    "journey_id": id,
                    "reason": "journey_cancelled_by_driver"
                }),
            );
        }
    }

    // 2. Cascade unlinking to any parcels assigned to this journey in PickupPending status
    let mut parcels = state.parcels.write().await;
    for parcel in parcels.values_mut() {
        if parcel.assigned_journey_id == Some(id)
            && parcel.status == waymesh_core::ParcelStatus::PickupPending
        {
            parcel.status = waymesh_core::ParcelStatus::Published;
            parcel.assigned_journey_id = None;
            parcel.carrier_id = None;
            parcel.pickup_otp_hash = None;
            parcel.delivery_otp_hash = None;
            if let Some(db) = &state.db {
                let _ = db.save_parcel(parcel).await;
            }
            state.emit_event(
                "parcel.republished",
                serde_json::json!({
                    "parcel_id": parcel.id,
                    "previous_journey_id": id,
                    "reason": "carrier_journey_cancelled"
                }),
            );
        }
    }

    Ok(Json(journey.clone()))
}

#[derive(Deserialize)]
pub struct SearchMatchesRequest {
    #[serde(alias = "origin_name")]
    pub pickup_name: Option<String>,
    #[serde(alias = "origin_lat", default)]
    pub pickup_lat: f64,
    #[serde(alias = "origin_lng", default)]
    pub pickup_lng: f64,
    #[serde(alias = "destination_name")]
    pub dropoff_name: Option<String>,
    #[serde(alias = "destination_lat", default)]
    pub dropoff_lat: f64,
    #[serde(alias = "destination_lng", default)]
    pub dropoff_lng: f64,
    #[serde(deserialize_with = "deserialize_flexible_datetime")]
    pub desired_time: DateTime<Utc>,
    pub demand_type: String, // "passenger" or "parcel"
    pub seats: Option<u32>,
    pub cargo_kg: Option<f64>,
    pub passenger_id: Option<Uuid>,
    #[serde(alias = "driver_gender", alias = "gender")]
    pub passenger_gender: Option<String>,
}

async fn search_matches(
    State(state): State<AppState>,
    Json(req): Json<SearchMatchesRequest>,
) -> impl IntoResponse {
    let journeys = state.journeys.read().await;

    let candidates: Vec<JourneyCandidate> = journeys
        .values()
        .filter(|j| j.status == JourneyStatus::Active)
        .map(|j| JourneyCandidate {
            journey_id: j.id,
            driver_id: j.driver_id,
            driver_gender: j.driver_gender,
            departure_time: j.departure_time_window_start,
            available_seats: j.available_seats,
            available_cargo_weight_kg: j.cargo_weight_capacity_kg,
            origin: j.origin_point.clone(),
            destination: j.destination_point.clone(),
            max_detour_minutes: j.flexible_detour_minutes,
            cost_per_seat: j.suggested_cost_per_seat,
        })
        .collect();

    let d_type = match req.demand_type.to_lowercase().as_str() {
        "parcel" => DemandType::ParcelDelivery,
        _ => DemandType::PassengerRide,
    };

    let p_gender = if let Some(g_str) = req.passenger_gender.as_deref() {
        Gender::from_str(g_str).ok()
    } else if let Some(pid) = req.passenger_id {
        let identities = state.identities.read().await;
        identities.get(&pid).map(|i| i.gender)
    } else {
        None
    };

    let pickup_point = if req.pickup_lat == 0.0 && req.pickup_lng == 0.0 {
        req.pickup_name
            .as_deref()
            .and_then(resolve_known_landmark)
            .unwrap_or(GeoPoint::new(req.pickup_lat, req.pickup_lng))
    } else {
        GeoPoint::new(req.pickup_lat, req.pickup_lng)
    };

    let dropoff_point = if req.dropoff_lat == 0.0 && req.dropoff_lng == 0.0 {
        req.dropoff_name
            .as_deref()
            .and_then(resolve_known_landmark)
            .unwrap_or(GeoPoint::new(req.dropoff_lat, req.dropoff_lng))
    } else {
        GeoPoint::new(req.dropoff_lat, req.dropoff_lng)
    };

    let demand = DemandItem {
        demand_id: Uuid::new_v4(),
        demand_type: d_type,
        passenger_gender: p_gender,
        pickup_location: pickup_point,
        dropoff_location: dropoff_point,
        ready_time: req.desired_time - chrono::Duration::hours(1),
        deadline_time: req.desired_time + chrono::Duration::hours(2),
        seats_required: req.seats.unwrap_or(1),
        cargo_weight_kg: req.cargo_kg.unwrap_or(0.0),
    };

    let problem = MatchProblem {
        journeys: candidates,
        demands: vec![demand],
        optimization_objective: Objective::MinimizeTotalDetour,
    };

    let solution =
        state
            .solver
            .solve(problem)
            .await
            .unwrap_or_else(|_| waymesh_solver::SolverSolution {
                assignments: Vec::new(),
                unassigned_demands: Vec::new(),
                solver_identifier: "error".into(),
                computation_time_ms: 0,
            });

    let mut sol_val = serde_json::to_value(&solution).unwrap_or_default();
    if solution.assignments.is_empty() {
        let requests = state.ride_requests.read().await;
        let waiting_corridor_requests: Vec<_> = requests
            .values()
            .filter(|r| r.status == RideRequestStatus::Published)
            .filter(|r| {
                if let Some(ref g) = p_gender {
                    r.passenger_gender.is_compatible_for_ride(g)
                } else {
                    true
                }
            })
            .map(|r| {
                serde_json::json!({
                    "request_id": r.id,
                    "origin_name": r.origin_name,
                    "destination_name": r.destination_name,
                    "desired_time": r.desired_time,
                    "passenger_gender": r.passenger_gender,
                    "seats": r.seats
                })
            })
            .collect();

        if !waiting_corridor_requests.is_empty() {
            sol_val["waiting_ride_requests"] = serde_json::json!(waiting_corridor_requests);
            sol_val["message"] = serde_json::json!(format!(
                "No driver journeys currently scheduled, but {} other commuter ride request(s) are active along this route. You can publish your own ride request via waymesh_publish_ride_request, or split/carpool together.",
                waiting_corridor_requests.len()
            ));
        } else {
            sol_val["message"] = serde_json::json!("No matching active journeys found. You can publish a ride request via waymesh_publish_ride_request so drivers along your route can see and accept it, or offer a ride via waymesh_publish_journey.");
        }
    }

    Json(sol_val)
}

// ----------------- Ride Request Handlers -----------------

#[derive(Deserialize)]
pub struct PublishRideRequestInput {
    pub passenger_id: Option<Uuid>,
    pub passenger_name: Option<String>,
    #[serde(alias = "gender")]
    pub passenger_gender: Option<String>,
    #[serde(alias = "pickup_name")]
    pub origin_name: String,
    #[serde(alias = "pickup_lat")]
    pub origin_lat: Option<f64>,
    #[serde(alias = "pickup_lng")]
    pub origin_lng: Option<f64>,
    #[serde(alias = "dropoff_name")]
    pub destination_name: String,
    #[serde(alias = "dropoff_lat")]
    pub destination_lat: Option<f64>,
    #[serde(alias = "dropoff_lng")]
    pub destination_lng: Option<f64>,
    #[serde(deserialize_with = "deserialize_flexible_datetime")]
    pub desired_time: DateTime<Utc>,
    pub time_window_minutes: Option<u32>,
    pub seats: Option<u32>,
    pub max_cost_share: Option<u32>,
}

async fn publish_ride_request(
    State(state): State<AppState>,
    Json(req): Json<PublishRideRequestInput>,
) -> impl IntoResponse {
    let (passenger_id, passenger_gender) = if let Some(pid) = req.passenger_id {
        let gender = if let Some(g_str) = req.passenger_gender.as_deref() {
            Gender::from_str(g_str).unwrap_or(Gender::Other)
        } else {
            let identities = state.identities.read().await;
            identities
                .get(&pid)
                .map(|id| id.gender)
                .unwrap_or(Gender::Other)
        };
        (pid, gender)
    } else {
        let gender = req
            .passenger_gender
            .as_deref()
            .and_then(|g| Gender::from_str(g).ok());

        let gender = match gender {
            Some(g) => g,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": "Gender specification ('male' or 'female') is required by the Safe Carpooling Protocol to guarantee passenger safety. Male and female commuters are never matched together in passenger rides."
                    })),
                );
            }
        };

        let name = req
            .passenger_name
            .unwrap_or_else(|| "Commuter Passenger".to_string());
        let keypair = waymesh_core::crypto::KeyPair::generate();
        let new_identity = Identity::new(keypair.public_key_hex(), name, gender);
        let pid = new_identity.id;
        {
            let mut identities = state.identities.write().await;
            identities.insert(pid, new_identity.clone());
        }
        if let Some(db) = &state.db {
            let _ = db.save_identity(&new_identity).await;
        }
        (pid, gender)
    };

    let origin_lat = req.origin_lat.unwrap_or(0.0);
    let origin_lng = req.origin_lng.unwrap_or(0.0);
    let origin_point = if origin_lat == 0.0 && origin_lng == 0.0 {
        match resolve_known_landmark(&req.origin_name) {
            Some(pt) => pt,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": format!("Could not resolve origin location '{}'. Please use a recognized landmark (e.g. CET, Technopark, TVM Central Station, Lulu Mall, Airport, Kochuveli) or provide origin_lat and origin_lng.", req.origin_name)
                    })),
                );
            }
        }
    } else {
        GeoPoint::new(origin_lat, origin_lng)
    };

    let dest_lat = req.destination_lat.unwrap_or(0.0);
    let dest_lng = req.destination_lng.unwrap_or(0.0);
    let dest_point = if dest_lat == 0.0 && dest_lng == 0.0 {
        match resolve_known_landmark(&req.destination_name) {
            Some(pt) => pt,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": format!("Could not resolve destination location '{}'. Please use a recognized landmark (e.g. CET, Technopark, TVM Central Station, Lulu Mall, Airport, Kochuveli) or provide destination_lat and destination_lng.", req.destination_name)
                    })),
                );
            }
        }
    } else {
        GeoPoint::new(dest_lat, dest_lng)
    };

    let mut ride_request = RideRequest::new(
        passenger_id,
        passenger_gender,
        req.origin_name,
        origin_point,
        req.destination_name,
        dest_point,
        req.desired_time,
        req.seats.unwrap_or(1),
        req.max_cost_share,
    );
    if let Some(w) = req.time_window_minutes {
        ride_request.time_window_minutes = w;
    }

    // Check if any existing active journey matches this request immediately
    let heuristic = SpatialCorridorHeuristic::default();
    let mut immediate_matches = Vec::new();
    {
        let journeys = state.journeys.read().await;
        for j in journeys.values() {
            if j.status == JourneyStatus::Active
                && j.available_seats >= ride_request.seats
                && ride_request
                    .passenger_gender
                    .is_compatible_for_ride(&j.driver_gender)
            {
                if let Some((detour, score)) = heuristic.score_corridor_fit(
                    &j.origin_point,
                    &j.destination_point,
                    j.flexible_detour_minutes,
                    &ride_request.origin_point,
                    &ride_request.destination_point,
                ) {
                    immediate_matches.push(serde_json::json!({
                        "journey_id": j.id,
                        "driver_id": j.driver_id,
                        "driver_gender": j.driver_gender,
                        "origin_name": j.origin_name,
                        "destination_name": j.destination_name,
                        "departure_time": j.departure_time_window_start,
                        "available_seats": j.available_seats,
                        "cost_per_seat": j.suggested_cost_per_seat,
                        "detour_minutes": detour,
                        "match_score": score,
                    }));
                }
            }
        }
    }

    {
        let mut ride_requests = state.ride_requests.write().await;
        ride_requests.insert(ride_request.id, ride_request.clone());
    }

    if let Some(db) = &state.db {
        if let Err(e) = db.save_ride_request(&ride_request).await {
            tracing::error!("Failed to save ride request to db: {}", e);
        }
    }

    state.emit_event(
        "ride_request.published",
        serde_json::json!({
            "request_id": ride_request.id,
            "passenger_id": ride_request.passenger_id,
            "passenger_gender": ride_request.passenger_gender,
            "origin_name": ride_request.origin_name,
            "destination_name": ride_request.destination_name,
        }),
    );

    let mut response_val = serde_json::to_value(&ride_request).unwrap_or_default();
    response_val["immediate_matches"] = serde_json::json!(immediate_matches);
    response_val["message"] = serde_json::json!(format!(
        "Ride request published successfully! Waiting for matching drivers along route {} -> {}.",
        ride_request.origin_name, ride_request.destination_name
    ));

    (StatusCode::CREATED, Json(response_val))
}

async fn get_active_ride_requests(State(state): State<AppState>) -> impl IntoResponse {
    let requests = state.ride_requests.read().await;
    let active: Vec<RideRequest> = requests
        .values()
        .filter(|r| r.status == RideRequestStatus::Published)
        .cloned()
        .collect();
    Json(active)
}

#[derive(Deserialize)]
pub struct SearchRideRequestsInput {
    #[serde(alias = "pickup_name")]
    pub origin_name: Option<String>,
    #[serde(alias = "pickup_lat")]
    pub origin_lat: Option<f64>,
    #[serde(alias = "pickup_lng")]
    pub origin_lng: Option<f64>,
    #[serde(alias = "dropoff_name")]
    pub destination_name: Option<String>,
    #[serde(alias = "dropoff_lat")]
    pub destination_lat: Option<f64>,
    #[serde(alias = "dropoff_lng")]
    pub destination_lng: Option<f64>,
    #[serde(deserialize_with = "deserialize_flexible_datetime")]
    pub departure_time: DateTime<Utc>,
    pub driver_gender: Option<String>,
    pub max_detour_minutes: Option<u32>,
}

async fn search_ride_requests(
    State(state): State<AppState>,
    Json(req): Json<SearchRideRequestsInput>,
) -> impl IntoResponse {
    let heuristic = SpatialCorridorHeuristic::default();
    let max_detour = req.max_detour_minutes.unwrap_or(15);

    let orig_lat = req.origin_lat.unwrap_or(0.0);
    let orig_lng = req.origin_lng.unwrap_or(0.0);
    let driver_origin = if orig_lat == 0.0 && orig_lng == 0.0 {
        req.origin_name
            .as_deref()
            .and_then(resolve_known_landmark)
            .unwrap_or(GeoPoint::new(orig_lat, orig_lng))
    } else {
        GeoPoint::new(orig_lat, orig_lng)
    };

    let dest_lat = req.destination_lat.unwrap_or(0.0);
    let dest_lng = req.destination_lng.unwrap_or(0.0);
    let driver_dest = if dest_lat == 0.0 && dest_lng == 0.0 {
        req.destination_name
            .as_deref()
            .and_then(resolve_known_landmark)
            .unwrap_or(GeoPoint::new(dest_lat, dest_lng))
    } else {
        GeoPoint::new(dest_lat, dest_lng)
    };

    let d_gender = req
        .driver_gender
        .as_deref()
        .and_then(|g| Gender::from_str(g).ok());

    let requests = state.ride_requests.read().await;
    let mut matches = Vec::new();

    for r in requests.values() {
        if r.status != RideRequestStatus::Published {
            continue;
        }

        // Strict gender separation check: male and female never match in passenger rides
        if let Some(ref dg) = d_gender {
            if !r.passenger_gender.is_compatible_for_ride(dg) {
                continue;
            }
        }

        // Time window check (e.g. within 1 hour)
        let diff_minutes = (r.desired_time - req.departure_time).num_minutes().abs();
        if diff_minutes > (r.time_window_minutes.max(60) as i64) {
            continue;
        }

        if let Some((detour, score)) = heuristic.score_corridor_fit(
            &driver_origin,
            &driver_dest,
            max_detour,
            &r.origin_point,
            &r.destination_point,
        ) {
            matches.push(serde_json::json!({
                "request_id": r.id,
                "passenger_id": r.passenger_id,
                "passenger_gender": r.passenger_gender,
                "origin_name": r.origin_name,
                "destination_name": r.destination_name,
                "desired_time": r.desired_time,
                "seats": r.seats,
                "max_cost_share": r.max_cost_share,
                "detour_minutes": detour,
                "match_score": score,
            }));
        }
    }

    // Sort by best match score descending
    matches.sort_by(|a, b| {
        let sa = a["match_score"].as_f64().unwrap_or(0.0);
        let sb = b["match_score"].as_f64().unwrap_or(0.0);
        sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
    });

    Json(serde_json::json!({
        "matched_ride_requests": matches,
        "count": matches.len()
    }))
}

#[derive(Deserialize)]
pub struct AcceptRideRequestInput {
    pub driver_id: Option<Uuid>,
    pub driver_name: Option<String>,
    pub driver_gender: Option<String>,
    pub journey_id: Option<Uuid>,
    pub agreed_cost_share: Option<u32>,
}

async fn accept_ride_request(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<AcceptRideRequestInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let mut ride_requests = state.ride_requests.write().await;
    let ride_request = ride_requests.get_mut(&id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Ride request not found" })),
        )
    })?;

    if ride_request.status != RideRequestStatus::Published {
        return Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": "Ride request is no longer active" })),
        ));
    }

    // Determine driver identity
    let (driver_id, driver_gender) = if let Some(did) = req.driver_id {
        let gender = if let Some(g_str) = req.driver_gender.as_deref() {
            Gender::from_str(g_str).unwrap_or(Gender::Other)
        } else {
            let identities = state.identities.read().await;
            identities
                .get(&did)
                .map(|id| id.gender)
                .unwrap_or(Gender::Other)
        };
        (did, gender)
    } else {
        let gender = req
            .driver_gender
            .as_deref()
            .and_then(|g| Gender::from_str(g).ok())
            .unwrap_or(Gender::Other);
        let name = req
            .driver_name
            .unwrap_or_else(|| "Commuter Driver".to_string());
        let keypair = waymesh_core::crypto::KeyPair::generate();
        let new_identity = Identity::new(keypair.public_key_hex(), name, gender);
        let did = new_identity.id;
        {
            let mut identities = state.identities.write().await;
            identities.insert(did, new_identity.clone());
        }
        if let Some(db) = &state.db {
            let _ = db.save_identity(&new_identity).await;
        }
        (did, gender)
    };

    // Self-dealing prevention: Passenger cannot accept their own ride request as a driver
    if ride_request.passenger_id == driver_id {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "Self-dealing prohibited: Passenger cannot accept their own ride request as a driver"
            })),
        ));
    }

    // Strict gender separation check
    if !ride_request
        .passenger_gender
        .is_compatible_for_ride(&driver_gender)
    {
        tracing::warn!("Rejecting cross-gender ride fulfillment");
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "Gender separation policy violation: Male and female cannot match in passenger rides."
            })),
        ));
    }

    // Determine or create journey
    let (journey_id, journey) = if let Some(jid) = req.journey_id {
        let mut journeys = state.journeys.write().await;
        let j = journeys.get_mut(&jid).ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Journey not found" })),
            )
        })?;
        if !ride_request
            .passenger_gender
            .is_compatible_for_ride(&j.driver_gender)
        {
            return Err((
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": "Gender separation policy violation: Male and female cannot match in passenger rides."
                })),
            ));
        }
        j.reserve_seats(ride_request.seats).map_err(|_| {
            (
                StatusCode::CONFLICT,
                Json(serde_json::json!({ "error": "Not enough seats available on journey" })),
            )
        })?;
        if let Some(db) = &state.db {
            let _ = db.save_journey(j).await;
        }
        (jid, j.clone())
    } else {
        // Auto-create a journey for the driver matching the passenger's route
        let mut new_journey = Journey::new(
            driver_id,
            driver_gender,
            ride_request.origin_name.clone(),
            ride_request.origin_point.clone(),
            ride_request.destination_name.clone(),
            ride_request.destination_point.clone(),
            ride_request.desired_time,
            ride_request.desired_time + chrono::Duration::minutes(30),
            3,
            10.0,
        );
        new_journey.reserve_seats(ride_request.seats).map_err(|_| {
            (
                StatusCode::CONFLICT,
                Json(serde_json::json!({ "error": "Not enough seats available" })),
            )
        })?;
        let jid = new_journey.id;
        {
            let mut journeys = state.journeys.write().await;
            journeys.insert(jid, new_journey.clone());
        }
        if let Some(db) = &state.db {
            let _ = db.save_journey(&new_journey).await;
        }
        (jid, new_journey)
    };

    // Create booking for the passenger
    let cost = req
        .agreed_cost_share
        .or(ride_request.max_cost_share)
        .unwrap_or(journey.suggested_cost_per_seat);

    let mut booking = RideBooking::new(
        journey_id,
        ride_request.passenger_id,
        ride_request.passenger_gender,
        ride_request.seats,
        ride_request.origin_name.clone(),
        ride_request.origin_point.clone(),
        ride_request.destination_name.clone(),
        ride_request.destination_point.clone(),
        cost,
    );

    let otp = booking.accept().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Failed to accept booking" })),
        )
    })?;

    {
        let mut bookings = state.bookings.write().await;
        bookings.insert(booking.id, booking.clone());
    }

    if let Some(db) = &state.db {
        let _ = db.save_booking(&booking).await;
    }

    // Mark ride request matched
    ride_request
        .match_with_journey(journey_id, booking.id, Some(otp.clone()))
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "Failed to match ride request" })),
            )
        })?;

    if let Some(db) = &state.db {
        let _ = db
            .update_ride_request_status(
                id,
                RideRequestStatus::Matched,
                Some(journey_id),
                Some(booking.id),
            )
            .await;
    }

    state.emit_event(
        "ride_request.matched",
        serde_json::json!({
            "request_id": id,
            "journey_id": journey_id,
            "booking_id": booking.id,
            "driver_id": driver_id,
            "passenger_id": ride_request.passenger_id
        }),
    );

    Ok(Json(serde_json::json!({
        "ride_request": ride_request,
        "journey": journey,
        "booking": booking,
        "pickup_otp": otp
    })))
}

async fn get_ride_request_by_id(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    let ride_requests = state.ride_requests.read().await;
    let req = ride_requests.get(&id).ok_or(StatusCode::NOT_FOUND)?;

    let mut val = serde_json::to_value(req).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if req.status == RideRequestStatus::Matched {
        if let Some(ref otp) = req.pickup_otp {
            val["pickup_otp"] = serde_json::json!(otp);
        }
        if let Some(booking_id) = req.assigned_booking_id {
            let bookings = state.bookings.read().await;
            if let Some(booking) = bookings.get(&booking_id) {
                val["booking"] = serde_json::to_value(booking).unwrap_or_default();
            }
        }
        if let Some(journey_id) = req.assigned_journey_id {
            let journeys = state.journeys.read().await;
            if let Some(journey) = journeys.get(&journey_id) {
                val["journey"] = serde_json::to_value(journey).unwrap_or_default();
            }
        }
    }

    Ok(Json(val))
}

async fn cancel_ride_request(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut ride_requests = state.ride_requests.write().await;
    let ride_request = ride_requests.get_mut(&id).ok_or(StatusCode::NOT_FOUND)?;
    ride_request.cancel().map_err(|_| StatusCode::BAD_REQUEST)?;

    if let Some(db) = &state.db {
        let _ = db
            .update_ride_request_status(id, RideRequestStatus::Cancelled, None, None)
            .await;
    }

    state.emit_event(
        "ride_request.cancelled",
        serde_json::json!({ "request_id": id }),
    );
    Ok(Json(ride_request.clone()))
}

// ----------------- Booking Handlers -----------------

#[derive(Deserialize)]
pub struct CreateBookingRequest {
    pub journey_id: Uuid,
    pub passenger_id: Option<Uuid>,
    pub passenger_name: Option<String>,
    pub passenger_gender: Option<String>,
    pub seats: u32,
    #[serde(alias = "origin_name")]
    pub pickup_name: String,
    #[serde(default)]
    pub pickup_lat: f64,
    #[serde(default)]
    pub pickup_lng: f64,
    #[serde(alias = "destination_name")]
    pub dropoff_name: String,
    #[serde(default)]
    pub dropoff_lat: f64,
    #[serde(default)]
    pub dropoff_lng: f64,
    #[serde(alias = "cost_paise", alias = "cost")]
    pub agreed_cost_share: u32,
}

async fn create_booking(
    State(state): State<AppState>,
    Json(req): Json<CreateBookingRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let journeys = state.journeys.read().await;
    let journey = journeys.get(&req.journey_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Journey not found" })),
        )
    })?;

    let (passenger_id, passenger_gender) = if let Some(pid) = req.passenger_id {
        let p_gender = if let Some(g_str) = req.passenger_gender.as_deref() {
            Gender::from_str(g_str).unwrap_or(Gender::Other)
        } else {
            let identities = state.identities.read().await;
            identities
                .get(&pid)
                .map(|id| id.gender)
                .unwrap_or(Gender::Other)
        };
        (pid, p_gender)
    } else {
        // Conversational auto-provisioning: If passenger_id is omitted by ChatGPT/Claude, auto-generate identity
        let p_gender = req
            .passenger_gender
            .as_deref()
            .and_then(|g| Gender::from_str(g).ok())
            .unwrap_or(Gender::Other);
        let name = req
            .passenger_name
            .unwrap_or_else(|| "Conversational Commuter".to_string());
        let keypair = waymesh_core::crypto::KeyPair::generate();
        let new_identity = Identity::new(keypair.public_key_hex(), name, p_gender);
        let new_id = new_identity.id;
        {
            let mut identities = state.identities.write().await;
            identities.insert(new_id, new_identity.clone());
        }
        if let Some(db) = &state.db {
            let _ = db.save_identity(&new_identity).await;
        }
        (new_id, p_gender)
    };

    let pickup_point = if req.pickup_lat == 0.0 && req.pickup_lng == 0.0 {
        resolve_known_landmark(&req.pickup_name)
            .unwrap_or(GeoPoint::new(req.pickup_lat, req.pickup_lng))
    } else {
        GeoPoint::new(req.pickup_lat, req.pickup_lng)
    };
    let dropoff_point = if req.dropoff_lat == 0.0 && req.dropoff_lng == 0.0 {
        resolve_known_landmark(&req.dropoff_name)
            .unwrap_or(GeoPoint::new(req.dropoff_lat, req.dropoff_lng))
    } else {
        GeoPoint::new(req.dropoff_lat, req.dropoff_lng)
    };

    // Self-dealing prevention: Driver cannot book seats on their own journey
    if journey.driver_id == passenger_id {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "Self-dealing prohibited: Driver cannot book seats on their own journey"
            })),
        ));
    }

    let booking = RideBooking::new(
        req.journey_id,
        passenger_id,
        passenger_gender,
        req.seats,
        req.pickup_name,
        pickup_point,
        req.dropoff_name,
        dropoff_point,
        req.agreed_cost_share,
    );

    // Strict gender separation defense-in-depth: Male and Female must NEVER be matched in passenger rides
    booking
        .validate_gender_compatibility(&journey.driver_gender)
        .map_err(|e| {
            tracing::warn!("Gender separation policy violation rejected: {}", e);
            (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": format!("Gender separation policy violation: {}. Male and female cannot match in passenger rides.", e)
                })),
            )
        })?;

    let mut bookings = state.bookings.write().await;
    bookings.insert(booking.id, booking.clone());

    if let Some(db) = &state.db {
        if let Err(e) = db.save_booking(&booking).await {
            tracing::error!("Failed to persist booking {}: {}", booking.id, e);
        }
    }

    state.emit_event(
        "booking.created",
        serde_json::json!({ "booking_id": booking.id, "journey_id": booking.journey_id }),
    );

    Ok((StatusCode::CREATED, Json(booking)))
}

async fn accept_booking(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut bookings = state.bookings.write().await;
    let booking = bookings.get_mut(&id).ok_or(StatusCode::NOT_FOUND)?;

    // Atomically decrement seats on the journey
    let mut journeys = state.journeys.write().await;
    let journey = journeys
        .get_mut(&booking.journey_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    // Self-dealing prevention: Driver cannot accept a booking from themselves
    if journey.driver_id == booking.passenger_id {
        return Err(StatusCode::FORBIDDEN);
    }

    journey
        .reserve_seats(booking.seats_booked)
        .map_err(|_| StatusCode::CONFLICT)?;

    let otp = booking.accept().map_err(|_| StatusCode::BAD_REQUEST)?;

    if let Some(db) = &state.db {
        if let Err(e) = db.save_booking(booking).await {
            tracing::error!("Failed to update booking {}: {}", booking.id, e);
        }
        if let Err(e) = db.save_journey(journey).await {
            tracing::error!("Failed to update journey {}: {}", journey.id, e);
        }
    }

    state.emit_event(
        "booking.accepted",
        serde_json::json!({ "booking_id": id, "passenger_id": booking.passenger_id }),
    );

    Ok(Json(serde_json::json!({
        "booking": booking,
        "pickup_otp": otp
    })))
}

#[derive(Deserialize)]
pub struct VerifyOtpRequest {
    pub otp: String,
}

async fn verify_booking_pickup(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<VerifyOtpRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut bookings = state.bookings.write().await;
    let booking = bookings.get_mut(&id).ok_or(StatusCode::NOT_FOUND)?;
    booking
        .verify_pickup(&req.otp)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    if let Some(db) = &state.db {
        if let Err(e) = db.save_booking(booking).await {
            tracing::error!("Failed to update booking {}: {}", booking.id, e);
        }
    }

    state.emit_event(
        "booking.pickup_verified",
        serde_json::json!({ "booking_id": id }),
    );
    Ok(Json(booking.clone()))
}

async fn complete_booking(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut bookings = state.bookings.write().await;
    let booking = bookings.get_mut(&id).ok_or(StatusCode::NOT_FOUND)?;
    booking.complete().map_err(|_| StatusCode::BAD_REQUEST)?;

    if let Some(db) = &state.db {
        if let Err(e) = db.save_booking(booking).await {
            tracing::error!("Failed to update booking {}: {}", booking.id, e);
        }
    }

    state.emit_event("booking.completed", serde_json::json!({ "booking_id": id }));
    Ok(Json(booking.clone()))
}

async fn cancel_booking(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut bookings = state.bookings.write().await;
    let booking = bookings.get_mut(&id).ok_or(StatusCode::NOT_FOUND)?;

    // If seats were reserved, restore them
    if booking.status == waymesh_core::BookingStatus::Accepted {
        let mut journeys = state.journeys.write().await;
        if let Some(journey) = journeys.get_mut(&booking.journey_id) {
            journey.release_seats(booking.seats_booked);
            if let Some(db) = &state.db {
                let _ = db.save_journey(journey).await;
            }
        }
    }

    booking.cancel().map_err(|_| StatusCode::BAD_REQUEST)?;

    if let Some(db) = &state.db {
        if let Err(e) = db.save_booking(booking).await {
            tracing::error!("Failed to update booking {}: {}", booking.id, e);
        }
    }

    state.emit_event("booking.cancelled", serde_json::json!({ "booking_id": id }));
    Ok(Json(booking.clone()))
}

#[derive(Deserialize)]
pub struct CreateParcelRequest {
    pub sender_id: Option<Uuid>,
    pub sender_name: Option<String>,
    pub recipient_name: String,
    pub recipient_phone: String,
    #[serde(alias = "pickup_name", alias = "origin_name")]
    pub pickup_address: String,
    #[serde(default)]
    pub pickup_lat: f64,
    #[serde(default)]
    pub pickup_lng: f64,
    #[serde(
        alias = "dropoff_name",
        alias = "delivery_name",
        alias = "destination_name"
    )]
    pub dropoff_address: String,
    #[serde(default)]
    pub dropoff_lat: f64,
    #[serde(default)]
    pub dropoff_lng: f64,
    pub weight_kg: f64,
    pub volume_liters: Option<f64>,
    #[serde(
        alias = "declared_contents",
        alias = "description",
        alias = "package_contents"
    )]
    pub contents: String,
    #[serde(
        deserialize_with = "deserialize_flexible_datetime",
        alias = "deadline",
        alias = "delivery_deadline"
    )]
    pub deadline_time: DateTime<Utc>,
    #[serde(
        alias = "compensation_paise",
        alias = "offered_compensation",
        alias = "reward"
    )]
    pub compensation: u32,
}

async fn create_parcel(
    State(state): State<AppState>,
    Json(req): Json<CreateParcelRequest>,
) -> impl IntoResponse {
    let sender_id = if let Some(sid) = req.sender_id {
        sid
    } else {
        let name = req
            .sender_name
            .unwrap_or_else(|| "Parcel Sender".to_string());
        let keypair = waymesh_core::crypto::KeyPair::generate();
        let new_identity = Identity::new(keypair.public_key_hex(), name, Gender::Other);
        let sid = new_identity.id;
        {
            let mut identities = state.identities.write().await;
            identities.insert(sid, new_identity.clone());
        }
        if let Some(db) = &state.db {
            let _ = db.save_identity(&new_identity).await;
        }
        sid
    };

    let pickup_point = if req.pickup_lat == 0.0 && req.pickup_lng == 0.0 {
        resolve_known_landmark(&req.pickup_address)
            .unwrap_or(GeoPoint::new(req.pickup_lat, req.pickup_lng))
    } else {
        GeoPoint::new(req.pickup_lat, req.pickup_lng)
    };
    let dropoff_point = if req.dropoff_lat == 0.0 && req.dropoff_lng == 0.0 {
        resolve_known_landmark(&req.dropoff_address)
            .unwrap_or(GeoPoint::new(req.dropoff_lat, req.dropoff_lng))
    } else {
        GeoPoint::new(req.dropoff_lat, req.dropoff_lng)
    };

    let parcel = Parcel::new(
        sender_id,
        req.recipient_name,
        &req.recipient_phone,
        req.pickup_address,
        pickup_point,
        req.dropoff_address,
        dropoff_point,
        req.weight_kg,
        req.volume_liters.unwrap_or(5.0),
        req.contents,
        req.deadline_time,
        req.compensation,
    );

    let mut parcels = state.parcels.write().await;
    parcels.insert(parcel.id, parcel.clone());

    if let Some(db) = &state.db {
        if let Err(e) = db.save_parcel(&parcel).await {
            tracing::error!("Failed to persist parcel {}: {}", parcel.id, e);
        }
    }

    state.emit_event(
        "parcel.published",
        serde_json::json!({ "parcel_id": parcel.id, "sender_id": parcel.sender_id }),
    );

    (StatusCode::CREATED, Json(parcel))
}

#[derive(Deserialize)]
pub struct AcceptParcelRequest {
    pub carrier_id: Uuid,
    pub journey_id: Uuid,
}

async fn accept_parcel(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<AcceptParcelRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut parcels = state.parcels.write().await;
    let parcel = parcels.get_mut(&id).ok_or(StatusCode::NOT_FOUND)?;

    // Self-dealing prevention: Sender cannot accept to carry their own parcel
    if parcel.sender_id == req.carrier_id {
        return Err(StatusCode::FORBIDDEN);
    }

    let mut journeys = state.journeys.write().await;
    let journey = journeys
        .get_mut(&req.journey_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    journey
        .reserve_cargo(parcel.weight_kg, parcel.volume_liters)
        .map_err(|_| StatusCode::CONFLICT)?;

    let codes = parcel
        .accept(req.carrier_id, req.journey_id)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    if let Some(db) = &state.db {
        if let Err(e) = db.save_parcel(parcel).await {
            tracing::error!("Failed to update parcel {}: {}", parcel.id, e);
        }
        if let Err(e) = db.save_journey(journey).await {
            tracing::error!("Failed to update journey {}: {}", journey.id, e);
        }
    }

    state.emit_event(
        "parcel.accepted",
        serde_json::json!({ "parcel_id": id, "carrier_id": req.carrier_id }),
    );

    Ok(Json(serde_json::json!({
        "parcel": parcel,
        "pickup_otp": codes.pickup_otp,
        "delivery_otp": codes.delivery_otp
    })))
}

async fn verify_parcel_pickup(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<VerifyOtpRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut parcels = state.parcels.write().await;
    let parcel = parcels.get_mut(&id).ok_or(StatusCode::NOT_FOUND)?;
    parcel
        .verify_pickup(&req.otp)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    if let Some(db) = &state.db {
        if let Err(e) = db.save_parcel(parcel).await {
            tracing::error!("Failed to update parcel {}: {}", parcel.id, e);
        }
    }

    state.emit_event("parcel.picked_up", serde_json::json!({ "parcel_id": id }));
    Ok(Json(parcel.clone()))
}

async fn verify_parcel_deliver(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<VerifyOtpRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut parcels = state.parcels.write().await;
    let parcel = parcels.get_mut(&id).ok_or(StatusCode::NOT_FOUND)?;
    parcel
        .verify_delivery(&req.otp)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    if let Some(db) = &state.db {
        if let Err(e) = db.save_parcel(parcel).await {
            tracing::error!("Failed to update parcel {}: {}", parcel.id, e);
        }
    }

    state.emit_event("parcel.delivered", serde_json::json!({ "parcel_id": id }));
    Ok(Json(parcel.clone()))
}

async fn cancel_parcel(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut parcels = state.parcels.write().await;
    let parcel = parcels.get_mut(&id).ok_or(StatusCode::NOT_FOUND)?;

    if parcel.status == ParcelStatus::PickupPending {
        if let Some(journey_id) = parcel.assigned_journey_id {
            let mut journeys = state.journeys.write().await;
            if let Some(journey) = journeys.get_mut(&journey_id) {
                journey.release_cargo(parcel.weight_kg, parcel.volume_liters);
                if let Some(db) = &state.db {
                    let _ = db.save_journey(journey).await;
                }
            }
        }
    }

    parcel.cancel().map_err(|_| StatusCode::BAD_REQUEST)?;

    if let Some(db) = &state.db {
        if let Err(e) = db.save_parcel(parcel).await {
            tracing::error!("Failed to update parcel {}: {}", parcel.id, e);
        }
    }

    state.emit_event("parcel.cancelled", serde_json::json!({ "parcel_id": id }));
    Ok(Json(parcel.clone()))
}

async fn event_stream(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = std::result::Result<Event, Infallible>>> {
    let rx = state.event_bus.subscribe();
    let stream = tokio_stream::wrappers::BroadcastStream::new(rx);
    let event_stream = tokio_stream::StreamExt::filter_map(stream, |res| match res {
        Ok(msg) => Some(Ok(Event::default().data(msg))),
        Err(_) => None,
    });

    Sse::new(event_stream).keep_alive(KeepAlive::default())
}

// ----------------- Remote Streamable HTTP MCP & Plugin Handlers -----------------

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

async fn handle_mcp_get() -> impl IntoResponse {
    Json(serde_json::json!({
        "name": "waymesh-mcp",
        "version": "0.1.0",
        "transport": "streamable-http",
        "endpoint": "/mcp",
        "description": "Waymesh Decentralized Mobility & Logistics Remote MCP Server"
    }))
}

async fn handle_mcp_post(
    State(state): State<AppState>,
    Json(req): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    let req_id = req.id.clone().unwrap_or(serde_json::Value::Null);

    match req.method.as_str() {
        "initialize" => (
            StatusCode::OK,
            Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "waymesh-mcp",
                        "version": "0.1.0"
                    },
                    "instructions": "Waymesh Decentralized Mobility and Logistics Node. Enforces strict gender separation: male and female participants are never matched together in passenger rides. Zero-UUID conversational UX: provide friendly display names instead of raw IDs. Settle directly between parties privately via peer UPI, cash, or mutual arrangement. No crypto."
                }
            })),
        ),
        "notifications/initialized" => (
            StatusCode::OK,
            Json(serde_json::json!({ "jsonrpc": "2.0" })),
        ),
        "ping" => (
            StatusCode::OK,
            Json(serde_json::json!({ "jsonrpc": "2.0", "id": req_id, "result": {} })),
        ),
        "tools/list" => (
            StatusCode::OK,
            Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "result": {
                    "tools": get_mcp_tools_list()
                }
            })),
        ),
        "tools/call" => {
            let res = execute_mcp_tool_call(&state, req.params).await;
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": res
                })),
            )
        }
        _ => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "error": { "code": -32601, "message": format!("Method not found: {}", req.method) }
            })),
        ),
    }
}

fn get_mcp_tools_list() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "waymesh_search_matches",
            "description": "Search available rides or parcel carriers along a travel corridor. Strictly enforces gender separation (male and female never match in passenger rides). If no rides are found, passenger can publish a ride request via waymesh_publish_ride_request.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pickup_name": { "type": "string", "description": "Optional pickup landmark name (e.g. 'CET Trivandrum')" },
                    "pickup_lat": { "type": "number", "description": "Pickup latitude (e.g. 8.5458 for CET, auto-resolved if 0)" },
                    "pickup_lng": { "type": "number", "description": "Pickup longitude (e.g. 76.9063 for CET, auto-resolved if 0)" },
                    "dropoff_name": { "type": "string", "description": "Optional dropoff landmark name (e.g. 'TVM Central Railway Station')" },
                    "dropoff_lat": { "type": "number", "description": "Dropoff latitude (e.g. 8.4875 for TVM Station, auto-resolved if 0)" },
                    "dropoff_lng": { "type": "number", "description": "Dropoff longitude (e.g. 76.9525 for TVM Station, auto-resolved if 0)" },
                    "desired_time": { "type": "string", "format": "date-time" },
                    "demand_type": { "type": "string", "enum": ["passenger", "parcel"] },
                    "seats": { "type": "integer", "default": 1 },
                    "passenger_gender": { "type": "string", "enum": ["male", "female", "other"], "description": "Passenger gender (males only match with males, females only match with females)" }
                },
                "required": ["desired_time", "demand_type"]
            }
        },
        {
            "name": "waymesh_publish_ride_request",
            "description": "Publish a passenger ride request (commuter demand / wanted ride) when no matching driver ride is available. Drivers heading along your route can see your request and offer/fulfill it. Strictly enforces gender separation (males match only with males, females only with females).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "passenger_name": { "type": "string", "description": "Display name of passenger (e.g. 'CET Student', 'Rahul')" },
                    "passenger_gender": { "type": "string", "enum": ["male", "female", "other"], "description": "Passenger gender (strictly separated: males match with male drivers, females with female drivers)" },
                    "origin_name": { "type": "string", "description": "Departure location or landmark (e.g. 'CET, Sreekaryam')" },
                    "origin_lat": { "type": "number", "description": "Departure latitude (auto-resolved from known landmarks if omitted)" },
                    "origin_lng": { "type": "number", "description": "Departure longitude (auto-resolved from known landmarks if omitted)" },
                    "destination_name": { "type": "string", "description": "Destination landmark or station (e.g. 'TVM Central Railway Station')" },
                    "destination_lat": { "type": "number", "description": "Destination latitude (auto-resolved from known landmarks if omitted)" },
                    "destination_lng": { "type": "number", "description": "Destination longitude (auto-resolved from known landmarks if omitted)" },
                    "desired_time": { "type": "string", "format": "date-time", "description": "Desired travel time (ISO-8601)" },
                    "seats": { "type": "integer", "default": 1, "description": "Number of seats needed" },
                    "max_cost_share": { "type": "integer", "description": "Max fare/budget in minor currency units (e.g. 5000 = ₹50.00)" }
                },
                "required": ["origin_name", "destination_name", "desired_time", "passenger_gender"]
            }
        },
        {
            "name": "waymesh_search_ride_requests",
            "description": "Search open passenger ride requests along a corridor. Allows drivers or carpoolers to find passengers needing a ride along their route.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "origin_name": { "type": "string", "description": "Optional driver departure landmark (e.g. 'CET Trivandrum')" },
                    "origin_lat": { "type": "number", "description": "Driver departure latitude (auto-resolved from origin_name if 0)" },
                    "origin_lng": { "type": "number", "description": "Driver departure longitude (auto-resolved from origin_name if 0)" },
                    "destination_name": { "type": "string", "description": "Optional driver destination landmark (e.g. 'TVM Central Railway Station')" },
                    "destination_lat": { "type": "number", "description": "Driver destination latitude (auto-resolved from destination_name if 0)" },
                    "destination_lng": { "type": "number", "description": "Driver destination longitude (auto-resolved from destination_name if 0)" },
                    "departure_time": { "type": "string", "format": "date-time", "description": "Driver departure time" },
                    "driver_gender": { "type": "string", "enum": ["male", "female", "other"], "description": "Driver gender (strictly matches compatible passengers)" },
                    "max_detour_minutes": { "type": "integer", "default": 15 }
                }
            }
        },
        {
            "name": "waymesh_get_ride_request_status",
            "description": "Check the status of a passenger ride request. Returns whether it is Published (waiting for driver), Matched (accepted by driver, includes driver info, booking details, and the 6-digit pickup OTP), or Cancelled.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "request_id": { "type": "string", "description": "UUID of the published ride request" }
                },
                "required": ["request_id"]
            }
        },
        {
            "name": "waymesh_cancel_ride_request",
            "description": "Cancel an active passenger ride request.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "request_id": { "type": "string", "description": "UUID of the published ride request to cancel" }
                },
                "required": ["request_id"]
            }
        },
        {
            "name": "waymesh_accept_ride_request",
            "description": "Driver accepts/fulfills an open passenger ride request. Connects the driver and passenger, reserves seats, and generates the secret 6-digit pickup OTP.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "request_id": { "type": "string", "description": "UUID of the published ride request" },
                    "driver_name": { "type": "string", "description": "Display name of driver (e.g. 'Arun Kumar')" },
                    "driver_gender": { "type": "string", "enum": ["male", "female", "other"], "description": "Driver gender" },
                    "journey_id": { "type": "string", "description": "Optional UUID of existing journey; if omitted, journey is auto-created" },
                    "agreed_cost_share": { "type": "integer", "description": "Agreed fare in minor currency units (e.g. 5000 = ₹50.00)" }
                },
                "required": ["request_id"]
            }
        },
        {
            "name": "waymesh_publish_journey",
            "description": "Publish a planned commuter or driver journey with spare seats or cargo capacity. Automatically checks for matching passenger ride requests waiting along the corridor.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "driver_name": { "type": "string", "description": "Display name of driver (e.g. 'Sunita Rao')" },
                    "driver_gender": { "type": "string", "enum": ["male", "female", "other"] },
                    "driver_id": { "type": "string", "description": "Optional driver UUID" },
                    "origin_name": { "type": "string" },
                    "origin_lat": { "type": "number", "description": "Origin latitude (auto-resolved if 0)" },
                    "origin_lng": { "type": "number", "description": "Origin longitude (auto-resolved if 0)" },
                    "destination_name": { "type": "string" },
                    "destination_lat": { "type": "number", "description": "Destination latitude (auto-resolved if 0)" },
                    "destination_lng": { "type": "number", "description": "Destination longitude (auto-resolved if 0)" },
                    "departure_time": { "type": "string", "format": "date-time" },
                    "seats": { "type": "integer", "default": 3 },
                    "cargo_kg": { "type": "number", "default": 10.0 },
                    "suggested_cost_per_seat": { "type": "integer", "description": "Cost in minor units (4000 = ₹40.00)" }
                },
                "required": ["origin_name", "destination_name", "departure_time"]
            }
        },
        {
            "name": "waymesh_create_booking",
            "description": "Request a seat on a matched journey. Zero-UUID support: provide passenger_name and passenger_gender. Enforces strict gender separation.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "journey_id": { "type": "string", "description": "UUID of matched journey" },
                    "passenger_name": { "type": "string", "description": "Display name of passenger (e.g. 'Priya Nair', 'CET Student')" },
                    "passenger_gender": { "type": "string", "enum": ["male", "female", "other"] },
                    "seats": { "type": "integer", "default": 1 },
                    "pickup_name": { "type": "string" },
                    "pickup_lat": { "type": "number", "default": 0.0 },
                    "pickup_lng": { "type": "number", "default": 0.0 },
                    "dropoff_name": { "type": "string" },
                    "dropoff_lat": { "type": "number", "default": 0.0 },
                    "dropoff_lng": { "type": "number", "default": 0.0 },
                    "agreed_cost_share": { "type": "integer", "description": "Cost share in minor units (e.g. 4000 = ₹40.00)" }
                },
                "required": ["journey_id", "seats", "pickup_name", "dropoff_name", "agreed_cost_share"]
            }
        },
        {
            "name": "waymesh_accept_booking",
            "description": "Driver accepts a pending ride booking request and generates the secret 6-digit pickup OTP.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "booking_id": { "type": "string" }
                },
                "required": ["booking_id"]
            }
        },
        {
            "name": "waymesh_request_parcel_delivery",
            "description": "Request a peer traveler to transport a package along their journey. Zero-UUID support: sender_name auto-provisions identity.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "sender_name": { "type": "string", "description": "Display name of sender (e.g. 'Anjali Sharma')" },
                    "recipient_name": { "type": "string" },
                    "recipient_phone": { "type": "string" },
                    "pickup_address": { "type": "string" },
                    "pickup_lat": { "type": "number", "default": 0.0 },
                    "pickup_lng": { "type": "number", "default": 0.0 },
                    "dropoff_address": { "type": "string" },
                    "dropoff_lat": { "type": "number", "default": 0.0 },
                    "dropoff_lng": { "type": "number", "default": 0.0 },
                    "weight_kg": { "type": "number" },
                    "volume_liters": { "type": "number", "default": 5.0 },
                    "contents": { "type": "string" },
                    "deadline_time": { "type": "string", "format": "date-time" },
                    "compensation": { "type": "integer" }
                },
                "required": ["recipient_name", "recipient_phone", "pickup_address", "dropoff_address", "weight_kg", "contents", "deadline_time", "compensation"]
            }
        },
        {
            "name": "waymesh_accept_parcel",
            "description": "Carrier accepts a parcel delivery request, generating pickup and delivery OTPs.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "parcel_id": { "type": "string" },
                    "carrier_id": { "type": "string" },
                    "journey_id": { "type": "string" }
                },
                "required": ["parcel_id", "carrier_id", "journey_id"]
            }
        },
        {
            "name": "waymesh_verify_handover",
            "description": "Verify physical boarding or parcel delivery using the 6-digit cryptographic OTP code.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "entity_type": { "type": "string", "enum": ["booking", "parcel"] },
                    "entity_id": { "type": "string" },
                    "otp": { "type": "string" },
                    "action": { "type": "string", "enum": ["pickup", "deliver"] }
                },
                "required": ["entity_type", "entity_id", "otp", "action"]
            }
        },
        {
            "name": "waymesh_cancel_booking",
            "description": "Cancel an active or requested ride booking.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "booking_id": { "type": "string", "description": "UUID of booking to cancel" }
                },
                "required": ["booking_id"]
            }
        },
        {
            "name": "waymesh_cancel_journey",
            "description": "Cancel a published driver journey. Cascades cancellation to non-completed bookings and unlinks pickup-pending parcels.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "journey_id": { "type": "string", "description": "UUID of journey to cancel" }
                },
                "required": ["journey_id"]
            }
        },
        {
            "name": "waymesh_cancel_parcel",
            "description": "Cancel a published or pickup-pending parcel delivery request.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "parcel_id": { "type": "string", "description": "UUID of parcel to cancel" }
                },
                "required": ["parcel_id"]
            }
        },
        {
            "name": "waymesh_register_identity",
            "description": "Register a new user, driver, or carrier identity on the Waymesh node with Ed25519 public key and gender.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "public_key_ed25519": { "type": "string" },
                    "display_name": { "type": "string" },
                    "gender": { "type": "string", "enum": ["male", "female", "other"] }
                },
                "required": ["public_key_ed25519", "display_name"]
            }
        }
    ])
}

async fn execute_mcp_tool_call(
    state: &AppState,
    params: Option<serde_json::Value>,
) -> serde_json::Value {
    let params = match params {
        Some(p) => p,
        None => {
            return serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": "Missing params" }] })
        }
    };
    let tool_name = match params.get("name").and_then(|v| v.as_str()) {
        Some(n) => n,
        None => {
            return serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": "Missing tool name" }] })
        }
    };
    let mut args = params
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let normalize_datetime_arg = |args: &mut serde_json::Value, field: &str| {
        if let Some(val) = args.get(field).and_then(|v| v.as_str()) {
            if let Some(dt) = waymesh_core::parse_flexible_datetime(val) {
                args[field] = serde_json::json!(dt.to_rfc3339());
            }
        } else if args.get(field).is_none() || args[field].is_null() {
            args[field] = serde_json::json!(chrono::Utc::now().to_rfc3339());
        }
    };

    // Auto-map cost_per_seat -> suggested_cost_per_seat if present
    if let Some(cost) = args.get("cost_per_seat").and_then(|v| v.as_u64()) {
        if args.get("suggested_cost_per_seat").is_none() {
            args["suggested_cost_per_seat"] = serde_json::json!(cost);
        }
    }

    match tool_name {
        "waymesh_search_matches" => {
            normalize_datetime_arg(&mut args, "desired_time");
            if args.get("demand_type").is_none() {
                args["demand_type"] = serde_json::json!("passenger");
            }
            if args.get("pickup_lat").is_none() {
                args["pickup_lat"] = serde_json::json!(0.0);
            }
            if args.get("pickup_lng").is_none() {
                args["pickup_lng"] = serde_json::json!(0.0);
            }
            if args.get("dropoff_lat").is_none() {
                args["dropoff_lat"] = serde_json::json!(0.0);
            }
            if args.get("dropoff_lng").is_none() {
                args["dropoff_lng"] = serde_json::json!(0.0);
            }

            match serde_json::from_value::<SearchMatchesRequest>(args) {
                Ok(req) => {
                    let resp = search_matches(State(state.clone()), Json(req))
                        .await
                        .into_response();
                    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
                        .await
                        .unwrap_or_default();
                    let sol: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
                    serde_json::json!({
                        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&sol).unwrap_or_default() }]
                    })
                }
                Err(e) => {
                    serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": format!("Invalid arguments: {}", e) }] })
                }
            }
        }
        "waymesh_publish_ride_request" => {
            normalize_datetime_arg(&mut args, "desired_time");
            if args.get("origin_lat").is_none() {
                args["origin_lat"] = serde_json::json!(0.0);
            }
            if args.get("origin_lng").is_none() {
                args["origin_lng"] = serde_json::json!(0.0);
            }
            if args.get("destination_lat").is_none() {
                args["destination_lat"] = serde_json::json!(0.0);
            }
            if args.get("destination_lng").is_none() {
                args["destination_lng"] = serde_json::json!(0.0);
            }

            match serde_json::from_value::<PublishRideRequestInput>(args) {
                Ok(req) => {
                    let resp = publish_ride_request(State(state.clone()), Json(req))
                        .await
                        .into_response();
                    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
                        .await
                        .unwrap_or_default();
                    let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
                    serde_json::json!({
                        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&val).unwrap_or_default() }]
                    })
                }
                Err(e) => {
                    serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": format!("Invalid arguments: {}", e) }] })
                }
            }
        }
        "waymesh_search_ride_requests" => {
            normalize_datetime_arg(&mut args, "departure_time");
            if args.get("origin_lat").is_none() {
                args["origin_lat"] = serde_json::json!(0.0);
            }
            if args.get("origin_lng").is_none() {
                args["origin_lng"] = serde_json::json!(0.0);
            }
            if args.get("destination_lat").is_none() {
                args["destination_lat"] = serde_json::json!(0.0);
            }
            if args.get("destination_lng").is_none() {
                args["destination_lng"] = serde_json::json!(0.0);
            }

            match serde_json::from_value::<SearchRideRequestsInput>(args) {
                Ok(req) => {
                    let resp = search_ride_requests(State(state.clone()), Json(req))
                        .await
                        .into_response();
                    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
                        .await
                        .unwrap_or_default();
                    let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
                    serde_json::json!({
                        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&val).unwrap_or_default() }]
                    })
                }
                Err(e) => {
                    serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": format!("Invalid arguments: {}", e) }] })
                }
            }
        }
        "waymesh_accept_ride_request" => {
            let request_id = match args
                .get("request_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
            {
                Some(id) => id,
                None => {
                    return serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": "Invalid request_id UUID" }] })
                }
            };
            match serde_json::from_value::<AcceptRideRequestInput>(args) {
                Ok(req) => {
                    match accept_ride_request(State(state.clone()), Path(request_id), Json(req))
                        .await
                    {
                        Ok(resp) => {
                            let bytes =
                                axum::body::to_bytes(resp.into_response().into_body(), usize::MAX)
                                    .await
                                    .unwrap_or_default();
                            let val: serde_json::Value =
                                serde_json::from_slice(&bytes).unwrap_or_default();
                            serde_json::json!({
                                "content": [{ "type": "text", "text": serde_json::to_string_pretty(&val).unwrap_or_default() }]
                            })
                        }
                        Err((status, json_err)) => serde_json::json!({
                            "isError": true,
                            "content": [{ "type": "text", "text": format!("Accept ride request failed with HTTP status {}: {}", status, json_err.get("error").and_then(|v| v.as_str()).unwrap_or("")) }]
                        }),
                    }
                }
                Err(e) => {
                    serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": format!("Invalid arguments: {}", e) }] })
                }
            }
        }
        "waymesh_get_ride_request_status" => {
            let request_id = match args
                .get("request_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
            {
                Some(id) => id,
                None => {
                    return serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": "Invalid request_id UUID" }] })
                }
            };
            match get_ride_request_by_id(State(state.clone()), Path(request_id)).await {
                Ok(resp) => {
                    let bytes = axum::body::to_bytes(resp.into_response().into_body(), usize::MAX)
                        .await
                        .unwrap_or_default();
                    let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
                    serde_json::json!({
                        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&val).unwrap_or_default() }]
                    })
                }
                Err(code) => serde_json::json!({
                    "isError": true,
                    "content": [{ "type": "text", "text": format!("Ride request lookup failed with HTTP status {}", code) }]
                }),
            }
        }
        "waymesh_cancel_ride_request" => {
            let request_id = match args
                .get("request_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
            {
                Some(id) => id,
                None => {
                    return serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": "Invalid request_id UUID" }] })
                }
            };
            match cancel_ride_request(State(state.clone()), Path(request_id)).await {
                Ok(resp) => {
                    let bytes = axum::body::to_bytes(resp.into_response().into_body(), usize::MAX)
                        .await
                        .unwrap_or_default();
                    let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
                    serde_json::json!({
                        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&val).unwrap_or_default() }]
                    })
                }
                Err(code) => serde_json::json!({
                    "isError": true,
                    "content": [{ "type": "text", "text": format!("Cancel ride request failed with HTTP status {}", code) }]
                }),
            }
        }
        "waymesh_create_booking" => {
            if args.get("pickup_lat").is_none() {
                args["pickup_lat"] = serde_json::json!(0.0);
            }
            if args.get("pickup_lng").is_none() {
                args["pickup_lng"] = serde_json::json!(0.0);
            }
            if args.get("dropoff_lat").is_none() {
                args["dropoff_lat"] = serde_json::json!(0.0);
            }
            if args.get("dropoff_lng").is_none() {
                args["dropoff_lng"] = serde_json::json!(0.0);
            }

            match serde_json::from_value::<CreateBookingRequest>(args) {
                Ok(req) => match create_booking(State(state.clone()), Json(req)).await {
                    Ok(resp) => {
                        let bytes =
                            axum::body::to_bytes(resp.into_response().into_body(), usize::MAX)
                                .await
                                .unwrap_or_default();
                        let b: serde_json::Value =
                            serde_json::from_slice(&bytes).unwrap_or_default();
                        serde_json::json!({
                            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&b).unwrap_or_default() }]
                        })
                    }
                    Err((status, json_val)) => serde_json::json!({
                        "isError": true,
                        "content": [{ "type": "text", "text": format!("Booking rejected with HTTP status {}: {}", status, json_val.get("error").and_then(|v| v.as_str()).unwrap_or("")) }]
                    }),
                },
                Err(e) => {
                    serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": format!("Invalid arguments: {}", e) }] })
                }
            }
        }
        "waymesh_publish_journey" => {
            normalize_datetime_arg(&mut args, "departure_time");
            if args.get("origin_lat").is_none() {
                args["origin_lat"] = serde_json::json!(0.0);
            }
            if args.get("origin_lng").is_none() {
                args["origin_lng"] = serde_json::json!(0.0);
            }
            if args.get("destination_lat").is_none() {
                args["destination_lat"] = serde_json::json!(0.0);
            }
            if args.get("destination_lng").is_none() {
                args["destination_lng"] = serde_json::json!(0.0);
            }

            match serde_json::from_value::<PublishJourneyRequest>(args) {
                Ok(req) => {
                    let resp = publish_journey(State(state.clone()), Json(req))
                        .await
                        .into_response();
                    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
                        .await
                        .unwrap_or_default();
                    let j: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
                    serde_json::json!({
                        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&j).unwrap_or_default() }]
                    })
                }
                Err(e) => {
                    serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": format!("Invalid arguments: {}", e) }] })
                }
            }
        }
        "waymesh_accept_booking" => {
            let booking_id = match args
                .get("booking_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
            {
                Some(id) => id,
                None => {
                    return serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": "Invalid booking_id UUID" }] })
                }
            };
            match accept_booking(State(state.clone()), Path(booking_id)).await {
                Ok(resp) => {
                    let val = axum::body::to_bytes(resp.into_response().into_body(), usize::MAX)
                        .await
                        .unwrap_or_default();
                    let json_val: serde_json::Value =
                        serde_json::from_slice(&val).unwrap_or_default();
                    serde_json::json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&json_val).unwrap_or_default() }] })
                }
                Err(code) => {
                    serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": format!("Accept failed with code {}", code) }] })
                }
            }
        }
        "waymesh_request_parcel_delivery" => {
            normalize_datetime_arg(&mut args, "deadline_time");
            if args.get("pickup_lat").is_none() {
                args["pickup_lat"] = serde_json::json!(0.0);
            }
            if args.get("pickup_lng").is_none() {
                args["pickup_lng"] = serde_json::json!(0.0);
            }
            if args.get("dropoff_lat").is_none() {
                args["dropoff_lat"] = serde_json::json!(0.0);
            }
            if args.get("dropoff_lng").is_none() {
                args["dropoff_lng"] = serde_json::json!(0.0);
            }

            match serde_json::from_value::<CreateParcelRequest>(args) {
                Ok(req) => {
                    let resp = create_parcel(State(state.clone()), Json(req))
                        .await
                        .into_response();
                    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
                        .await
                        .unwrap_or_default();
                    let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
                    serde_json::json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&val).unwrap_or_default() }] })
                }
                Err(e) => {
                    serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": format!("Invalid arguments: {}", e) }] })
                }
            }
        }
        "waymesh_accept_parcel" => {
            let parcel_id = match args
                .get("parcel_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
            {
                Some(id) => id,
                None => {
                    return serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": "Invalid parcel_id UUID" }] })
                }
            };
            match serde_json::from_value::<AcceptParcelRequest>(args) {
                Ok(req) => {
                    match accept_parcel(State(state.clone()), Path(parcel_id), Json(req)).await {
                        Ok(resp) => {
                            let bytes =
                                axum::body::to_bytes(resp.into_response().into_body(), usize::MAX)
                                    .await
                                    .unwrap_or_default();
                            let val: serde_json::Value =
                                serde_json::from_slice(&bytes).unwrap_or_default();
                            serde_json::json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&val).unwrap_or_default() }] })
                        }
                        Err(code) => {
                            serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": format!("Accept parcel failed with code {}", code) }] })
                        }
                    }
                }
                Err(e) => {
                    serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": format!("Invalid arguments: {}", e) }] })
                }
            }
        }
        "waymesh_verify_handover" => {
            let entity_type = args
                .get("entity_type")
                .and_then(|v| v.as_str())
                .unwrap_or("booking");
            let entity_id = match args
                .get("entity_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
            {
                Some(id) => id,
                None => {
                    return serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": "Invalid entity_id UUID" }] })
                }
            };
            let raw_otp = args.get("otp").and_then(|v| v.as_str()).unwrap_or("");
            let clean_otp: String = raw_otp.chars().filter(|c| c.is_ascii_digit()).collect();
            let action = args
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("pickup");

            if entity_type == "parcel" {
                if action == "deliver" {
                    match verify_parcel_deliver(
                        State(state.clone()),
                        Path(entity_id),
                        Json(VerifyOtpRequest { otp: clean_otp }),
                    )
                    .await
                    {
                        Ok(_) => {
                            serde_json::json!({ "content": [{ "type": "text", "text": "Parcel delivery handover verified successfully!" }] })
                        }
                        Err(code) => {
                            serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": format!("Verification failed: {}", code) }] })
                        }
                    }
                } else {
                    match verify_parcel_pickup(
                        State(state.clone()),
                        Path(entity_id),
                        Json(VerifyOtpRequest { otp: clean_otp }),
                    )
                    .await
                    {
                        Ok(_) => {
                            serde_json::json!({ "content": [{ "type": "text", "text": "Parcel pickup handover verified successfully!" }] })
                        }
                        Err(code) => {
                            serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": format!("Verification failed: {}", code) }] })
                        }
                    }
                }
            } else {
                match verify_booking_pickup(
                    State(state.clone()),
                    Path(entity_id),
                    Json(VerifyOtpRequest { otp: clean_otp }),
                )
                .await
                {
                    Ok(_) => {
                        serde_json::json!({ "content": [{ "type": "text", "text": "Passenger pickup OTP verified successfully! Ride commenced." }] })
                    }
                    Err(code) => {
                        serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": format!("Pickup verification failed: {}", code) }] })
                    }
                }
            }
        }
        "waymesh_register_identity" => {
            match serde_json::from_value::<RegisterIdentityRequest>(args) {
                Ok(req) => {
                    let resp = register_identity(State(state.clone()), Json(req))
                        .await
                        .into_response();
                    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
                        .await
                        .unwrap_or_default();
                    let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
                    serde_json::json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&val).unwrap_or_default() }] })
                }
                Err(e) => {
                    serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": format!("Invalid arguments: {}", e) }] })
                }
            }
        }
        "waymesh_cancel_booking" => {
            let booking_id = match args
                .get("booking_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
            {
                Some(id) => id,
                None => {
                    return serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": "Invalid booking_id UUID" }] })
                }
            };
            match cancel_booking(State(state.clone()), Path(booking_id)).await {
                Ok(resp) => {
                    let bytes = axum::body::to_bytes(resp.into_response().into_body(), usize::MAX)
                        .await
                        .unwrap_or_default();
                    let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
                    serde_json::json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&val).unwrap_or_default() }] })
                }
                Err(code) => {
                    serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": format!("Cancel booking failed with HTTP status {}", code) }] })
                }
            }
        }
        "waymesh_cancel_journey" => {
            let journey_id = match args
                .get("journey_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
            {
                Some(id) => id,
                None => {
                    return serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": "Invalid journey_id UUID" }] })
                }
            };
            match cancel_journey(State(state.clone()), Path(journey_id)).await {
                Ok(resp) => {
                    let bytes = axum::body::to_bytes(resp.into_response().into_body(), usize::MAX)
                        .await
                        .unwrap_or_default();
                    let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
                    serde_json::json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&val).unwrap_or_default() }] })
                }
                Err(code) => {
                    serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": format!("Cancel journey failed with HTTP status {}", code) }] })
                }
            }
        }
        "waymesh_cancel_parcel" => {
            let parcel_id = match args
                .get("parcel_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
            {
                Some(id) => id,
                None => {
                    return serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": "Invalid parcel_id UUID" }] })
                }
            };
            match cancel_parcel(State(state.clone()), Path(parcel_id)).await {
                Ok(resp) => {
                    let bytes = axum::body::to_bytes(resp.into_response().into_body(), usize::MAX)
                        .await
                        .unwrap_or_default();
                    let val: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
                    serde_json::json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&val).unwrap_or_default() }] })
                }
                Err(code) => {
                    serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": format!("Cancel parcel failed with HTTP status {}", code) }] })
                }
            }
        }
        _ => {
            serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": format!("Unknown tool: {}", tool_name) }] })
        }
    }
}

async fn serve_ai_plugin_manifest(State(state): State<AppState>) -> impl IntoResponse {
    let manifest = serde_json::json!({
        "schema_version": "v1",
        "name_for_human": "Waymesh Mobility",
        "name_for_model": "waymesh",
        "description_for_human": "Peer-to-peer ride sharing, safe carpooling, and crowdshipping logistics on the Waymesh open protocol.",
        "description_for_model": "Open protocol for searching and booking peer rides, safe carpooling, and parcel hitchhiking along travel corridors. Strictly enforces same-gender ride matching (males with males, females with females). Settle directly between parties privately via UPI, cash, or mutual arrangement.",
        "auth": { "type": "none" },
        "api": { "type": "openapi", "url": format!("{}/openapi.yaml", state.server_url) },
        "logo_url": format!("{}/logo.png", state.server_url),
        "contact_email": "support@icvalley.tech",
        "legal_info_url": format!("{}/health", state.server_url)
    });
    Json(manifest)
}

async fn serve_openapi_yaml(State(state): State<AppState>) -> impl IntoResponse {
    let template = include_str!("../openapi.yaml");
    let dynamically_rendered = template
        .replace("https://waymesh.icvalley.tech", &state.server_url)
        .replace("http://localhost:8080", &state.server_url);
    ([("content-type", "application/yaml")], dynamically_rendered)
}

// ----------------- Campus OAuth Handlers (@cet.ac.in restriction) -----------------

#[derive(Debug, Deserialize)]
pub struct OAuthAuthorizeQuery {
    pub redirect_uri: Option<String>,
    pub state: Option<String>,
    pub client_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OAuthAuthorizeResponse {
    pub auth_url: String,
    pub campus_domain: String,
    pub message: String,
}

async fn oauth_authorize(
    State(state): State<AppState>,
    Query(query): Query<OAuthAuthorizeQuery>,
) -> impl IntoResponse {
    let client_id = query
        .client_id
        .or_else(|| std::env::var("GOOGLE_CLIENT_ID").ok())
        .unwrap_or_else(|| "waymesh-cet-client-id".into());
    let redirect_uri = query
        .redirect_uri
        .or_else(|| std::env::var("GOOGLE_REDIRECT_URI").ok())
        .unwrap_or_else(|| format!("{}/api/v1/auth/oauth/callback", state.server_url));
    let csrf_state = query
        .state
        .unwrap_or_else(|| Uuid::new_v4().simple().to_string());

    // Google OAuth 2.0 authorization URL with hd=cet.ac.in to restrict account chooser
    let domain_clean = state.campus_domain.trim_start_matches('@');
    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope=openid%20email%20profile&hd={}&state={}",
        client_id,
        redirect_uri,
        domain_clean,
        csrf_state
    );

    Json(OAuthAuthorizeResponse {
        auth_url,
        campus_domain: state.campus_domain.clone(),
        message: format!(
            "Only verified {} student/faculty institutional accounts are authorized",
            state.campus_domain
        ),
    })
}

#[derive(Debug, Deserialize)]
pub struct OAuthCallbackRequest {
    pub code: String,
    pub redirect_uri: Option<String>,
    pub state: Option<String>,
    // Fallback fields for testing / simulated login
    pub email: Option<String>,
    pub name: Option<String>,
    pub sub: Option<String>,
    pub gender: Option<Gender>,
}

#[derive(Debug, Deserialize)]
pub struct OAuthTokenExchangeRequest {
    pub email: String,
    pub name: Option<String>,
    pub sub: Option<String>,
    pub provider: Option<String>,
    pub gender: Option<Gender>,
    pub id_token: Option<String>,
}

async fn oauth_callback(
    State(state): State<AppState>,
    Json(req): Json<OAuthCallbackRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let google_client_secret = std::env::var("GOOGLE_CLIENT_SECRET").ok();
    let google_client_id = std::env::var("GOOGLE_CLIENT_ID").ok();

    let (email, name, sub) = if let (Some(client_secret), Some(client_id)) =
        (google_client_secret, google_client_id)
    {
        let redirect_uri = req
            .redirect_uri
            .unwrap_or_else(|| format!("{}/api/v1/auth/oauth/callback", state.server_url));

        let token_resp = state
            .http_client
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("code", req.code.as_str()),
                ("client_id", client_id.as_str()),
                ("client_secret", client_secret.as_str()),
                ("redirect_uri", redirect_uri.as_str()),
                ("grant_type", "authorization_code"),
            ])
            .send()
            .await
            .map_err(|e| {
                (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({ "error": format!("OAuth token request failed: {}", e) })),
                )
            })?;

        let token_data: serde_json::Value = token_resp.json().await.map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": format!("Failed to parse Google OAuth token response: {}", e) })),
            )
        })?;

        let access_token = token_data
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({ "error": "No access_token returned by Google OAuth", "details": token_data })),
                )
            })?;

        let userinfo_resp = state
            .http_client
            .get("https://www.googleapis.com/oauth2/v3/userinfo")
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| {
                (
                    StatusCode::BAD_GATEWAY,
                    Json(
                        serde_json::json!({ "error": format!("Failed to fetch userinfo: {}", e) }),
                    ),
                )
            })?;

        let userinfo: serde_json::Value = userinfo_resp.json().await.map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": format!("Failed to parse userinfo: {}", e) })),
            )
        })?;

        let email = userinfo
            .get("email")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": "Google profile did not contain an email address" })),
                )
            })?
            .to_string();

        let name = userinfo
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let sub = userinfo
            .get("sub")
            .and_then(|v| v.as_str())
            .unwrap_or("google_user")
            .to_string();

        (email, name, sub)
    } else {
        // Dev / test mode fallback when remote Google exchange is not configured
        let email = req.email.ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "No email provided in callback payload and GOOGLE_CLIENT_SECRET is not configured for remote exchange"
                })),
            )
        })?;
        let sub = req.sub.unwrap_or_else(|| format!("sub_{}", req.code));
        (email, req.name, sub)
    };

    provision_or_login_oauth_user(&state, &email, name.as_deref(), &sub, "google", req.gender).await
}

async fn oauth_token_exchange(
    State(state): State<AppState>,
    Json(req): Json<OAuthTokenExchangeRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let provider = req.provider.unwrap_or_else(|| "google".into());
    let sub = req
        .sub
        .unwrap_or_else(|| format!("sub_{}", Uuid::new_v4().simple()));
    provision_or_login_oauth_user(
        &state,
        &req.email,
        req.name.as_deref(),
        &sub,
        &provider,
        req.gender,
    )
    .await
}

async fn provision_or_login_oauth_user(
    state: &AppState,
    email: &str,
    name: Option<&str>,
    sub: &str,
    provider: &str,
    gender: Option<Gender>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let email_normalized = email.trim().to_lowercase();

    // STRICT CAMPUS DOMAIN ENFORCEMENT: Only @cet.ac.in authorized
    if !email_normalized.ends_with(&state.campus_domain) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": format!(
                    "Unauthorized domain: email '{}' is not authorized. Access is strictly restricted to verified '{}' campus accounts.",
                    email_normalized, state.campus_domain
                ),
                "campus_domain": state.campus_domain,
                "status": 403
            })),
        ));
    }

    let mut identities = state.identities.write().await;
    let existing_id = identities
        .values()
        .find(|id| {
            id.email
                .as_deref()
                .map(|e| e.eq_ignore_ascii_case(&email_normalized))
                .unwrap_or(false)
                || (id.oauth_sub.as_deref() == Some(sub)
                    && id.oauth_provider.as_deref() == Some(provider))
        })
        .map(|id| id.id);

    let identity = if let Some(id) = existing_id {
        let id_entry = identities.get_mut(&id).unwrap();
        id_entry.is_verified = true;
        id_entry.email = Some(email_normalized.clone());
        id_entry.oauth_provider = Some(provider.to_string());
        id_entry.oauth_sub = Some(sub.to_string());
        if let Some(n) = name {
            id_entry.display_name = n.to_string();
        }
        if let Some(g) = gender {
            id_entry.gender = g;
        }
        id_entry.updated_at = Utc::now();
        id_entry.clone()
    } else {
        let keypair = waymesh_core::crypto::KeyPair::generate();
        let display_name = name
            .unwrap_or_else(|| email_normalized.split('@').next().unwrap_or("CET Student"))
            .to_string();
        let new_identity = Identity::with_oauth(
            keypair.public_key_hex(),
            display_name,
            gender.unwrap_or(Gender::Other),
            email_normalized.clone(),
            provider.to_string(),
            sub.to_string(),
        )
        .map_err(|e| {
            (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?;
        identities.insert(new_identity.id, new_identity.clone());
        new_identity
    };

    if let Some(db) = &state.db {
        if let Err(e) = db.save_identity(&identity).await {
            tracing::error!("Failed to save verified OAuth identity to db: {}", e);
        }
    }

    // Generate authenticated session token
    let session_token = format!("wm_cet_{}", Uuid::new_v4().simple());
    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(session_token.clone(), identity.id);
    }

    state.emit_event(
        "auth.verified",
        serde_json::json!({
            "identity_id": identity.id,
            "email": identity.email,
            "provider": provider,
            "domain": state.campus_domain,
        }),
    );

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "access_token": session_token,
            "token_type": "Bearer",
            "campus_domain": state.campus_domain,
            "identity": identity,
        })),
    ))
}

async fn get_current_authenticated_user(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "Missing Authorization header" })),
            )
        })?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .map(|s| s.trim())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "Invalid token format. Expected 'Bearer <token>'" })),
            )
        })?;

    let user_id = {
        let sessions = state.sessions.read().await;
        sessions.get(token).copied().ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "Invalid or expired session token" })),
            )
        })?
    };

    let identities = state.identities.read().await;
    let identity = identities.get(&user_id).cloned().ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "User identity not found" })),
        )
    })?;

    Ok(Json(serde_json::json!({
        "identity": identity,
        "authenticated": true,
        "campus_domain": state.campus_domain,
        "is_cet_verified": identity.is_cet_verified(),
    })))
}
