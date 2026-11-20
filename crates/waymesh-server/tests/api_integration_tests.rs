use axum::http::StatusCode;
use chrono::Utc;
use serde_json::json;
use tower::ServiceExt;
use waymesh_core::crypto::KeyPair;
use waymesh_server::{create_router, AppState};

#[tokio::test]
async fn test_full_ride_and_parcel_handover_flow() {
    let state = AppState::new();
    let app = create_router(state);

    // 1. Register Driver
    let driver_keys = KeyPair::generate();
    let driver_pk = driver_keys.public_key_hex();

    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/identities")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "public_key_ed25519": driver_pk,
                "display_name": "Rajesh (Campus Driver)"
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let driver: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    let driver_id = driver["id"].as_str().unwrap();

    // 2. Register Passenger
    let pass_keys = KeyPair::generate();
    let pass_pk = pass_keys.public_key_hex();

    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/identities")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "public_key_ed25519": pass_pk,
                "display_name": "Priya (Student Passenger)"
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let passenger: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    let passenger_id = passenger["id"].as_str().unwrap();

    // 3. Driver publishes journey from Campus Gate to Central Railway Station
    let now = Utc::now();
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/journeys")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "driver_id": driver_id,
                "origin_name": "Campus Gate 1",
                "origin_lat": 12.9716,
                "origin_lng": 77.5946,
                "destination_name": "City Central Railway Station",
                "destination_lat": 12.9780,
                "destination_lng": 77.5696,
                "departure_time": now + chrono::Duration::hours(1),
                "seats": 3,
                "cargo_kg": 15.0,
                "suggested_cost_per_seat": 8000
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let journey: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    let journey_id = journey["id"].as_str().unwrap();

    // 4. Passenger searches for matching ride along corridor
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/matches/search")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "pickup_lat": 12.9720,
                "pickup_lng": 77.5930,
                "dropoff_lat": 12.9775,
                "dropoff_lng": 77.5700,
                "desired_time": now + chrono::Duration::hours(1),
                "demand_type": "passenger",
                "seats": 1
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let search_res: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(search_res["assignments"].as_array().unwrap().len(), 1);

    // 5. Passenger creates booking
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/bookings")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "journey_id": journey_id,
                "passenger_id": passenger_id,
                "seats": 1,
                "pickup_name": "Hostel Junction",
                "pickup_lat": 12.9720,
                "pickup_lng": 77.5930,
                "dropoff_name": "Platform 1 Gate",
                "dropoff_lat": 12.9775,
                "dropoff_lng": 77.5700,
                "agreed_cost_share": 8000
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let booking: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    let booking_id = booking["id"].as_str().unwrap();

    // 6. Driver accepts booking (generates pickup OTP)
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/v1/bookings/{}/accept", booking_id))
        .header("content-type", "application/json")
        .body(axum::body::Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let accept_res: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    let pickup_otp = accept_res["pickup_otp"].as_str().unwrap();
    assert_eq!(pickup_otp.len(), 6);

    // 7. Driver verifies pickup OTP
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/v1/bookings/{}/verify-pickup", booking_id))
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({ "otp": pickup_otp }).to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 8. Driver completes ride at destination
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/v1/bookings/{}/complete", booking_id))
        .header("content-type", "application/json")
        .body(axum::body::Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 9. Parcel Delivery Flow
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/parcels")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "sender_id": passenger_id,
                "recipient_name": "Kavita",
                "recipient_phone": "+919876543210",
                "pickup_address": "Campus Post Office",
                "pickup_lat": 12.9718,
                "pickup_lng": 77.5940,
                "dropoff_address": "Station Cloakroom",
                "dropoff_lat": 12.9782,
                "dropoff_lng": 77.5690,
                "weight_kg": 2.5,
                "contents": "Study materials",
                "deadline_time": now + chrono::Duration::hours(3),
                "compensation": 10000
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let parcel: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    let parcel_id = parcel["id"].as_str().unwrap();

    // 10. Carrier accepts parcel (generates dual OTPs)
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/v1/parcels/{}/accept", parcel_id))
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "carrier_id": driver_id,
                "journey_id": journey_id
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let parcel_accept_res: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    let parcel_pickup_otp = parcel_accept_res["pickup_otp"].as_str().unwrap();
    let parcel_delivery_otp = parcel_accept_res["delivery_otp"].as_str().unwrap();

    // 11. Verify parcel pickup
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/v1/parcels/{}/pickup", parcel_id))
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({ "otp": parcel_pickup_otp }).to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 12. Verify parcel delivery
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/v1/parcels/{}/deliver", parcel_id))
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({ "otp": parcel_delivery_otp }).to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_strict_gender_separation_enforcement() {
    let state = AppState::new();
    let app = create_router(state);
    let now = Utc::now();

    // 1. Register Male Driver Rajesh
    let rajesh_keys = KeyPair::generate();
    let res = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/identities")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({
                        "public_key_ed25519": rajesh_keys.public_key_hex(),
                        "display_name": "Rajesh (Male Driver)",
                        "gender": "male"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let rajesh: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let rajesh_id = rajesh["id"].as_str().unwrap();

    // 2. Register Female Passenger Priya
    let priya_keys = KeyPair::generate();
    let res = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/identities")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({
                        "public_key_ed25519": priya_keys.public_key_hex(),
                        "display_name": "Priya (Female Passenger)",
                        "gender": "female"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let priya: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let priya_id = priya["id"].as_str().unwrap();

    // 3. Register Male Passenger Rohan
    let rohan_keys = KeyPair::generate();
    let res = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/identities")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({
                        "public_key_ed25519": rohan_keys.public_key_hex(),
                        "display_name": "Rohan (Male Passenger)",
                        "gender": "male"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let rohan: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let rohan_id = rohan["id"].as_str().unwrap();

    // 4. Register Female Driver Sunita
    let sunita_keys = KeyPair::generate();
    let res = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/identities")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({
                        "public_key_ed25519": sunita_keys.public_key_hex(),
                        "display_name": "Sunita (Female Driver)",
                        "gender": "female"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let sunita: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let sunita_id = sunita["id"].as_str().unwrap();

    // 5. Rajesh publishes journey (Male Driver)
    let res = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/journeys")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({
                        "driver_id": rajesh_id,
                        "driver_gender": "male",
                        "origin_name": "Campus Gate 1",
                        "origin_lat": 12.9716,
                        "origin_lng": 77.5946,
                        "destination_name": "City Central Railway Station",
                        "destination_lat": 12.9780,
                        "destination_lng": 77.5696,
                        "departure_time": now + chrono::Duration::hours(1),
                        "seats": 3,
                        "cargo_kg": 15.0,
                        "suggested_cost_per_seat": 8000
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let rajesh_journey: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let rajesh_journey_id = rajesh_journey["id"].as_str().unwrap();

    // 6. Female Passenger Priya searches for matching ride -> MUST RETURN 0 MATCHES
    let res = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/matches/search")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({
                        "pickup_lat": 12.9720,
                        "pickup_lng": 77.5930,
                        "dropoff_lat": 12.9775,
                        "dropoff_lng": 77.5700,
                        "desired_time": now + chrono::Duration::hours(1),
                        "demand_type": "passenger",
                        "seats": 1,
                        "passenger_id": priya_id,
                        "passenger_gender": "female"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let search_res: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        search_res["assignments"].as_array().unwrap().len(),
        0,
        "Female passenger must NOT match Male driver Rajesh"
    );
    assert_eq!(
        search_res["unassigned_demands"].as_array().unwrap().len(),
        1
    );

    // 7. Female Passenger Priya attempts to book Rajesh's journey directly -> MUST RETURN 403 FORBIDDEN
    let res = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/bookings")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({
                        "journey_id": rajesh_journey_id,
                        "passenger_id": priya_id,
                        "passenger_gender": "female",
                        "seats": 1,
                        "pickup_name": "Hostel Junction",
                        "pickup_lat": 12.9720,
                        "pickup_lng": 77.5930,
                        "dropoff_name": "Platform 1 Gate",
                        "dropoff_lat": 12.9775,
                        "dropoff_lng": 77.5700,
                        "agreed_cost_share": 8000
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::FORBIDDEN,
        "Booking must be rejected when genders are incompatible"
    );

    // 8. Male Passenger Rohan searches for matching ride -> MUST MATCH Male Driver Rajesh
    let res = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/matches/search")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({
                        "pickup_lat": 12.9720,
                        "pickup_lng": 77.5930,
                        "dropoff_lat": 12.9775,
                        "dropoff_lng": 77.5700,
                        "desired_time": now + chrono::Duration::hours(1),
                        "demand_type": "passenger",
                        "seats": 1,
                        "passenger_id": rohan_id,
                        "passenger_gender": "male"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let search_res: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        search_res["assignments"].as_array().unwrap().len(),
        1,
        "Male passenger must match Male driver Rajesh"
    );

    // 9. Male Passenger Rohan books Male Driver Rajesh's journey -> SUCCEEDS
    let res = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/bookings")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({
                        "journey_id": rajesh_journey_id,
                        "passenger_id": rohan_id,
                        "passenger_gender": "male",
                        "seats": 1,
                        "pickup_name": "Hostel Junction",
                        "pickup_lat": 12.9720,
                        "pickup_lng": 77.5930,
                        "dropoff_name": "Platform 1 Gate",
                        "dropoff_lat": 12.9775,
                        "dropoff_lng": 77.5700,
                        "agreed_cost_share": 8000
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // 10. Female Driver Sunita publishes journey
    let res = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/journeys")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({
                        "driver_id": sunita_id,
                        "driver_gender": "female",
                        "origin_name": "Campus Gate 1",
                        "origin_lat": 12.9716,
                        "origin_lng": 77.5946,
                        "destination_name": "City Central Railway Station",
                        "destination_lat": 12.9780,
                        "destination_lng": 77.5696,
                        "departure_time": now + chrono::Duration::hours(1),
                        "seats": 3,
                        "cargo_kg": 15.0,
                        "suggested_cost_per_seat": 8000
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let sunita_journey: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let sunita_journey_id = sunita_journey["id"].as_str().unwrap();

    // 11. Female Passenger Priya searches for matching ride -> MUST MATCH Female Driver Sunita
    let res = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/matches/search")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({
                        "pickup_lat": 12.9720,
                        "pickup_lng": 77.5930,
                        "dropoff_lat": 12.9775,
                        "dropoff_lng": 77.5700,
                        "desired_time": now + chrono::Duration::hours(1),
                        "demand_type": "passenger",
                        "seats": 1,
                        "passenger_id": priya_id,
                        "passenger_gender": "female"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let search_res: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        search_res["assignments"].as_array().unwrap().len(),
        1,
        "Female passenger must match Female driver Sunita"
    );

    // 12. Female Passenger Priya books Female Driver Sunita's journey -> SUCCEEDS
    let res = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/bookings")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({
                        "journey_id": sunita_journey_id,
                        "passenger_id": priya_id,
                        "passenger_gender": "female",
                        "seats": 1,
                        "pickup_name": "Hostel Junction",
                        "pickup_lat": 12.9720,
                        "pickup_lng": 77.5930,
                        "dropoff_name": "Platform 1 Gate",
                        "dropoff_lat": 12.9775,
                        "dropoff_lng": 77.5700,
                        "agreed_cost_share": 8000
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // 13. Parcel Delivery is Gender-Agnostic: Rajesh (Male) can carry parcel for Priya (Female)
    let res = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/parcels")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({
                        "sender_id": priya_id,
                        "recipient_name": "Kavita",
                        "recipient_phone": "+919876543210",
                        "pickup_address": "Campus Post Office",
                        "pickup_lat": 12.9718,
                        "pickup_lng": 77.5940,
                        "dropoff_address": "Station Cloakroom",
                        "dropoff_lat": 12.9782,
                        "dropoff_lng": 77.5690,
                        "weight_kg": 2.5,
                        "contents": "Study materials",
                        "deadline_time": now + chrono::Duration::hours(3),
                        "compensation": 10000
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let parcel: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let parcel_id = parcel["id"].as_str().unwrap();

    // Rajesh accepts parcel
    let res = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/api/v1/parcels/{}/accept", parcel_id))
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({
                        "carrier_id": rajesh_id,
                        "journey_id": rajesh_journey_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "Parcel logistics must succeed regardless of gender"
    );
}

#[tokio::test]
async fn test_conversational_zero_uuid_booking_flow() {
    let state = AppState::new();
    let app = create_router(state);
    let now = Utc::now();

    // 1. Female driver Sunita publishes journey
    let sunita_keys = KeyPair::generate();
    let res = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/identities")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({
                        "public_key_ed25519": sunita_keys.public_key_hex(),
                        "display_name": "Sunita Rao (Driver)",
                        "gender": "female"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let sunita: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let sunita_id = sunita["id"].as_str().unwrap();

    let res = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/journeys")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({
                        "driver_id": sunita_id,
                        "driver_gender": "female",
                        "origin_name": "College of Engineering Trivandrum (CET)",
                        "origin_lat": 8.5458,
                        "origin_lng": 76.9063,
                        "destination_name": "Kochuveli Railway Station (KCVL)",
                        "destination_lat": 8.5085,
                        "destination_lng": 76.9038,
                        "departure_time": now + chrono::Duration::hours(1),
                        "seats": 3,
                        "cargo_kg": 10.0,
                        "suggested_cost_per_seat": 4000
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let journey: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let journey_id = journey["id"].as_str().unwrap();

    // 2. Female passenger books WITHOUT supplying any passenger_id (Zero-UUID conversational UX)
    let res = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/bookings")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    json!({
                        "journey_id": journey_id,
                        "passenger_name": "CET Student",
                        "passenger_gender": "female",
                        "seats": 1,
                        "pickup_name": "CET Main Gate",
                        "pickup_lat": 8.5458,
                        "pickup_lng": 76.9063,
                        "dropoff_name": "Kochuveli Railway Station",
                        "dropoff_lat": 8.5085,
                        "dropoff_lng": 76.9038,
                        "agreed_cost_share": 4000
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "Conversational booking without UUID must auto-provision identity and succeed"
    );
    let booking: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(booking["passenger_id"].is_string());
    assert_eq!(booking["status"], "Requested");

    // 3. Driver accepts booking and generates OTP
    let booking_id = booking["id"].as_str().unwrap();
    let res = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/api/v1/bookings/{}/accept", booking_id))
                .header("content-type", "application/json")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let accept_res: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let otp = accept_res["pickup_otp"].as_str().unwrap();
    assert_eq!(otp.len(), 6);
}

#[tokio::test]
async fn test_ride_request_lifecycle_and_driver_matching() {
    let state = AppState::new();
    let app = create_router(state);

    let now = Utc::now();

    // 1. Passenger searches for ride CET -> TVM Station (no driver journeys exist yet)
    let search_req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/matches/search")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "pickup_name": "CET, Sreekaryam",
                "pickup_lat": 8.5458,
                "pickup_lng": 76.9063,
                "dropoff_name": "TVM Central Railway Station",
                "dropoff_lat": 8.4875,
                "dropoff_lng": 76.9525,
                "desired_time": now + chrono::Duration::hours(1),
                "demand_type": "passenger",
                "passenger_gender": "male",
                "seats": 1
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(search_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let search_res: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(search_res["assignments"].as_array().unwrap().len(), 0);
    assert!(search_res["message"]
        .as_str()
        .unwrap()
        .contains("waymesh_publish_ride_request"));

    // 2. Commuter publishes a passenger ride request ("just publish")
    let pub_req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/ride-requests")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "passenger_name": "Rahul (CET Commuter)",
                "passenger_gender": "male",
                "origin_name": "College of Engineering Trivandrum (CET)",
                "origin_lat": 0.0, // Auto-resolved by landmark resolution!
                "origin_lng": 0.0,
                "destination_name": "TVM Central Railway Station",
                "destination_lat": 0.0, // Auto-resolved by landmark resolution!
                "destination_lng": 0.0,
                "desired_time": now + chrono::Duration::hours(1),
                "seats": 1,
                "max_cost_share": 5000
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(pub_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let pub_res: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let request_id = pub_res["id"].as_str().unwrap();
    assert_eq!(pub_res["status"], "Published");
    // Verify auto-resolved coordinates
    assert!((pub_res["origin_point"]["latitude"].as_f64().unwrap() - 8.5458).abs() < 0.01);
    assert!((pub_res["destination_point"]["latitude"].as_f64().unwrap() - 8.4875).abs() < 0.01);

    // 3. Driver searches for open ride requests along route
    let search_rr_req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/ride-requests/search")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "origin_lat": 8.5460,
                "origin_lng": 76.9060,
                "destination_lat": 8.4870,
                "destination_lng": 76.9530,
                "departure_time": now + chrono::Duration::hours(1),
                "driver_gender": "male"
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(search_rr_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let matched_rr: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(matched_rr["count"], 1);
    assert_eq!(
        matched_rr["matched_ride_requests"][0]["request_id"],
        request_id
    );

    // 4. Male driver accepts/fulfills the ride request
    let accept_req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/v1/ride-requests/{}/accept", request_id))
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "driver_name": "Anand (CET Alum Driver)",
                "driver_gender": "male"
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(accept_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let fulfill_res: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();

    assert_eq!(fulfill_res["ride_request"]["status"], "Matched");
    let otp = fulfill_res["pickup_otp"].as_str().unwrap();
    assert_eq!(otp.len(), 6);
    let booking_id = fulfill_res["booking"]["id"].as_str().unwrap();

    // 5. Verify physical boarding with OTP
    let verify_req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/v1/bookings/{}/verify-pickup", booking_id))
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "otp": otp
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(verify_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_ride_request_strict_gender_separation() {
    let state = AppState::new();
    let app = create_router(state);

    let now = Utc::now();

    // Female passenger publishes a ride request
    let pub_req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/ride-requests")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "passenger_name": "Ananya",
                "passenger_gender": "female",
                "origin_name": "CET, Sreekaryam",
                "origin_lat": 8.5458,
                "origin_lng": 76.9063,
                "destination_name": "TVM Central Railway Station",
                "destination_lat": 8.4875,
                "destination_lng": 76.9525,
                "desired_time": now + chrono::Duration::hours(1),
                "seats": 1
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(pub_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let pub_res: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let request_id = pub_res["id"].as_str().unwrap();

    // Male driver attempts to search -> should NOT find female request
    let search_req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/ride-requests/search")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "origin_lat": 8.5458,
                "origin_lng": 76.9063,
                "destination_lat": 8.4875,
                "destination_lng": 76.9525,
                "departure_time": now + chrono::Duration::hours(1),
                "driver_gender": "male"
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(search_req).await.unwrap();
    let search_res: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(search_res["count"], 0);

    // Male driver attempts to accept female ride request -> HTTP 403 Forbidden
    let accept_req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/v1/ride-requests/{}/accept", request_id))
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "driver_name": "Rohan",
                "driver_gender": "male"
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(accept_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // Female driver accepts -> SUCCESS!
    let female_accept_req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/v1/ride-requests/{}/accept", request_id))
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "driver_name": "Divya",
                "driver_gender": "female"
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(female_accept_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_ride_request_status_lookup_and_cancellation() {
    let state = AppState::new();
    let app = create_router(state);
    let now = Utc::now();

    // 1. Publish ride request using landmark names without coords
    let pub_req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/ride-requests")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "passenger_name": "Arjun",
                "passenger_gender": "male",
                "origin_name": "CET",
                "destination_name": "TVM Station",
                "desired_time": now + chrono::Duration::hours(1),
                "seats": 1
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(pub_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let pub_res: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let request_id = pub_res["id"].as_str().unwrap();

    // 2. Lookup ride request by ID -> should be Published
    let get_req = axum::http::Request::builder()
        .method("GET")
        .uri(format!("/api/v1/ride-requests/{}", request_id))
        .body(axum::body::Body::empty())
        .unwrap();

    let res = app.clone().oneshot(get_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let get_res: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(get_res["status"], "Published");

    // 3. Driver searches using landmark names
    let search_req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/ride-requests/search")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "origin_name": "CET",
                "destination_name": "TVM Central Railway Station",
                "departure_time": now + chrono::Duration::hours(1),
                "driver_gender": "male"
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(search_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let search_res: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(search_res["count"], 1);

    // 4. Male driver accepts
    let accept_req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/v1/ride-requests/{}/accept", request_id))
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "driver_name": "Gokul",
                "driver_gender": "male"
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(accept_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let accept_res: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let otp = accept_res["pickup_otp"].as_str().unwrap();
    assert_eq!(otp.len(), 6);

    // 5. Lookup again by ID -> should be Matched with pickup_otp
    let get_req2 = axum::http::Request::builder()
        .method("GET")
        .uri(format!("/api/v1/ride-requests/{}", request_id))
        .body(axum::body::Body::empty())
        .unwrap();

    let res = app.clone().oneshot(get_req2).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let get_res2: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(get_res2["status"], "Matched");
    assert_eq!(get_res2["pickup_otp"], otp);

    // 6. Test cancellation of another ride request
    let pub_req2 = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/ride-requests")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "passenger_name": "Suresh",
                "passenger_gender": "male",
                "origin_name": "Technopark",
                "destination_name": "East Fort",
                "desired_time": now + chrono::Duration::hours(1),
                "seats": 1
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(pub_req2).await.unwrap();
    let pub_res2: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let request_id2 = pub_res2["id"].as_str().unwrap();

    let cancel_req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/v1/ride-requests/{}/cancel", request_id2))
        .body(axum::body::Body::empty())
        .unwrap();

    let res = app.clone().oneshot(cancel_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let cancel_res: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(cancel_res["status"], "Cancelled");
}

#[tokio::test]
async fn test_chavadimukku_to_tvm_station_zero_coord_flow() {
    let state = AppState::new();
    let app = create_router(state);

    // 1. Search with human string time and landmark names (no lat/lng provided)
    let search_req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/matches/search")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "origin_name": "chavadimuk",
                "destination_name": "tvm railway station",
                "demand_type": "passenger",
                "passenger_name": "Rider",
                "passenger_gender": "male",
                "seats": 1,
                "desired_time": "today 5:20 pm"
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(search_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let search_res: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(search_res["assignments"].as_array().unwrap().len(), 0);
    assert!(search_res["message"]
        .as_str()
        .unwrap()
        .contains("No matching active journeys found"));

    // 2. Publish Ride Request when no match is found
    let pub_req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/ride-requests")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "passenger_name": "Sahal",
                "passenger_gender": "male",
                "origin_name": "chavadimuk",
                "destination_name": "tvm railway station",
                "desired_time": "today 5:20 pm",
                "seats": 1
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(pub_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let pub_res: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(pub_res["status"], "Published");
    assert_eq!(pub_res["origin_name"], "chavadimuk");
    assert!((pub_res["origin_point"]["latitude"].as_f64().unwrap() - 8.5372).abs() < 0.001);
    assert!((pub_res["destination_point"]["latitude"].as_f64().unwrap() - 8.4875).abs() < 0.001);
}

#[tokio::test]
async fn test_cascading_journey_cancellation() {
    let state = AppState::new();
    let app = create_router(state.clone());

    // 1. Driver publishes journey
    let now = Utc::now();
    let journey_req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/journeys")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "driver_name": "Ramesh",
                "driver_gender": "male",
                "origin_name": "CET Trivandrum",
                "destination_name": "tvm railway station",
                "departure_time": now + chrono::Duration::hours(2),
                "seats": 3,
                "cargo_kg": 20.0,
                "suggested_cost_per_seat": 8000
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(journey_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let j_val: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let journey_id = j_val["id"].as_str().unwrap();

    // 2. Create and accept a booking
    let book_req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/bookings")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "journey_id": journey_id,
                "passenger_name": "Kiran",
                "passenger_gender": "male",
                "seats": 1,
                "pickup_name": "CET Gate",
                "dropoff_name": "Central Station",
                "agreed_cost_share": 8000
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(book_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let b_val: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let booking_id = b_val["id"].as_str().unwrap();

    let accept_b_req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/v1/bookings/{}/accept", booking_id))
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app.clone().oneshot(accept_b_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 3. Create and accept a parcel
    let parcel_req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/parcels")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "recipient_name": "Ajay",
                "recipient_phone": "+919999988888",
                "pickup_address": "CET Lab",
                "dropoff_address": "Station Parcel Office",
                "weight_kg": 2.5,
                "contents": "Document folder",
                "deadline_time": now + chrono::Duration::hours(3),
                "compensation": 10000
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(parcel_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let p_val: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let parcel_id = p_val["id"].as_str().unwrap();

    let accept_p_req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/v1/parcels/{}/accept", parcel_id))
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "carrier_id": j_val["driver_id"].as_str().unwrap(),
                "journey_id": journey_id
            })
            .to_string(),
        ))
        .unwrap();
    let res = app.clone().oneshot(accept_p_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4. Cancel Journey
    let cancel_j_req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/v1/journeys/{}/cancel", journey_id))
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app.clone().oneshot(cancel_j_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 5. Verify cascading state in memory
    let b_uuid = uuid::Uuid::parse_str(booking_id).unwrap();
    let p_uuid = uuid::Uuid::parse_str(parcel_id).unwrap();

    let bookings = state.bookings.read().await;
    let b = bookings.get(&b_uuid).unwrap();
    assert_eq!(b.status, waymesh_core::BookingStatus::Cancelled);

    let parcels = state.parcels.read().await;
    let p = parcels.get(&p_uuid).unwrap();
    assert_eq!(p.status, waymesh_core::ParcelStatus::Published);
    assert_eq!(p.assigned_journey_id, None);
    assert_eq!(p.carrier_id, None);
}

#[tokio::test]
async fn test_mcp_cancellation_tools() {
    let state = AppState::new();
    let app = create_router(state.clone());

    // 1. Verify tools/list contains cancellation tools
    let list_req = axum::http::Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/list"
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(list_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let list_val: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let tools = list_val["result"]["tools"].as_array().unwrap();
    let tool_names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(tool_names.contains(&"waymesh_cancel_booking"));
    assert!(tool_names.contains(&"waymesh_cancel_journey"));
    assert!(tool_names.contains(&"waymesh_cancel_parcel"));

    // 2. Publish journey & create booking
    let journey_req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/journeys")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "driver_name": "Deepa",
                "driver_gender": "female",
                "origin_name": "CET Trivandrum",
                "destination_name": "Lulu Mall Trivandrum",
                "departure_time": "in 1 hour",
                "seats": 2,
                "cargo_kg": 10.0,
                "suggested_cost_per_seat": 5000
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(journey_req).await.unwrap();
    let j_val: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let journey_id = j_val["id"].as_str().unwrap();

    let book_req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/bookings")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "journey_id": journey_id,
                "passenger_name": "Ananya",
                "passenger_gender": "female",
                "seats": 1,
                "pickup_name": "CET",
                "dropoff_name": "Lulu Mall",
                "agreed_cost_share": 5000
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(book_req).await.unwrap();
    let b_val: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let booking_id = b_val["id"].as_str().unwrap();

    // 3. Cancel booking via MCP tools/call
    let mcp_call_req = axum::http::Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {
                    "name": "waymesh_cancel_booking",
                    "arguments": {
                        "booking_id": booking_id
                    }
                }
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(mcp_call_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let call_val: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(call_val["result"]["isError"].is_null());

    let b_uuid = uuid::Uuid::parse_str(booking_id).unwrap();
    let bookings = state.bookings.read().await;
    assert_eq!(
        bookings.get(&b_uuid).unwrap().status,
        waymesh_core::BookingStatus::Cancelled
    );
}

#[tokio::test]
async fn test_oauth_cet_student_flow_and_session() {
    let state = AppState::new();
    let app = create_router(state);

    // 1. Check OAuth authorize endpoint
    let auth_req = axum::http::Request::builder()
        .method("GET")
        .uri("/api/v1/auth/oauth/authorize")
        .body(axum::body::Body::empty())
        .unwrap();

    let res = app.clone().oneshot(auth_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let auth_val: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert!(auth_val["auth_url"]
        .as_str()
        .unwrap()
        .contains("hd=cet.ac.in"));
    assert_eq!(auth_val["campus_domain"].as_str().unwrap(), "@cet.ac.in");

    // 2. Successful CET Student OAuth token exchange
    let token_req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/auth/oauth/token")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "email": "arjun.cs21@cet.ac.in",
                "name": "Arjun CS",
                "gender": "male",
                "provider": "google",
                "sub": "google_oauth_sub_12345"
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(token_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let token_val: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    let access_token = token_val["access_token"].as_str().unwrap();
    assert!(access_token.starts_with("wm_cet_"));
    assert_eq!(
        token_val["identity"]["email"].as_str().unwrap(),
        "arjun.cs21@cet.ac.in"
    );
    assert!(token_val["identity"]["is_verified"].as_bool().unwrap());

    // 3. Verify session via /api/v1/auth/me
    let me_req = axum::http::Request::builder()
        .method("GET")
        .uri("/api/v1/auth/me")
        .header("authorization", format!("Bearer {}", access_token))
        .body(axum::body::Body::empty())
        .unwrap();

    let res = app.clone().oneshot(me_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let me_val: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert!(me_val["authenticated"].as_bool().unwrap());
    assert!(me_val["is_cet_verified"].as_bool().unwrap());
    assert_eq!(
        me_val["identity"]["email"].as_str().unwrap(),
        "arjun.cs21@cet.ac.in"
    );
}

#[tokio::test]
async fn test_oauth_non_cet_email_strictly_rejected() {
    let state = AppState::new();
    let app = create_router(state);

    // Attempt token exchange with outsider gmail account
    let token_req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/auth/oauth/token")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "email": "outsider@gmail.com",
                "name": "Random Person",
                "provider": "google",
                "sub": "fake_sub"
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(token_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let err_val: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert!(err_val["error"].as_str().unwrap().contains("@cet.ac.in"));
}

#[tokio::test]
async fn test_self_dealing_strictly_prevented() {
    let state = AppState::new();
    let app = create_router(state);

    let driver_keys = KeyPair::generate();
    let driver_pk = driver_keys.public_key_hex();

    // Register user
    let reg_req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/identities")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "public_key_ed25519": driver_pk,
                "display_name": "Siddharth",
                "gender": "male"
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(reg_req).await.unwrap();
    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let user: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    let user_id = user["id"].as_str().unwrap();

    // 1. Self-Dealing in Ride Booking: Driver attempts to book their own journey
    let journey_req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/journeys")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "driver_id": user_id,
                "origin_name": "CET Main Gate",
                "destination_name": "Trivandrum Central",
                "departure_time": Utc::now() + chrono::Duration::hours(1),
                "seats": 3,
                "cargo_kg": 10.0,
                "suggested_cost_per_seat": 5000
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(journey_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let j_val: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let journey_id = j_val["id"].as_str().unwrap();

    let self_booking_req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/bookings")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "journey_id": journey_id,
                "passenger_id": user_id,
                "seats": 1,
                "pickup_name": "CET Main Gate",
                "dropoff_name": "Trivandrum Central",
                "agreed_cost_share": 5000
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(self_booking_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 2. Self-Dealing in Parcel Acceptance: Sender attempts to accept their own parcel
    let parcel_req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/parcels")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "sender_id": user_id,
                "recipient_name": "Rahul",
                "recipient_phone": "+919876543210",
                "pickup_name": "CET Dept",
                "dropoff_name": "Thampanoor",
                "weight_kg": 2.0,
                "description": "Lab Manuals",
                "deadline": Utc::now() + chrono::Duration::hours(3),
                "reward": 2000
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(parcel_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let p_val: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let parcel_id = p_val["id"].as_str().unwrap();

    let self_accept_parcel_req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/v1/parcels/{}/accept", parcel_id))
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "carrier_id": user_id,
                "journey_id": journey_id
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(self_accept_parcel_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 3. Self-Dealing in Ride Requests: Passenger attempts to accept their own ride request as a driver
    let ride_req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/ride-requests")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "passenger_id": user_id,
                "passenger_gender": "male",
                "origin_name": "CET Men's Hostel",
                "destination_name": "Technopark Phase 1",
                "desired_time": Utc::now() + chrono::Duration::hours(2),
                "seats": 1,
                "max_cost_share": 4000
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(ride_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let rr_val: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let request_id = rr_val["id"].as_str().unwrap();

    let self_accept_ride_req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/v1/ride-requests/{}/accept", request_id))
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "driver_id": user_id,
                "origin_name": "CET Men's Hostel",
                "destination_name": "Technopark Phase 1",
                "departure_time": Utc::now() + chrono::Duration::hours(2),
                "seats": 2,
                "suggested_cost_per_seat": 4000
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(self_accept_ride_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_dynamic_server_url_rendering() {
    std::env::set_var("SERVER_URL", "https://waymesh.cet.ac.in");
    let state = AppState::new();
    let app = create_router(state);

    let req = axum::http::Request::builder()
        .method("GET")
        .uri("/.well-known/ai-plugin.json")
        .body(axum::body::Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let val: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(
        val["api"]["url"].as_str().unwrap(),
        "https://waymesh.cet.ac.in/openapi.yaml"
    );

    let openapi_req = axum::http::Request::builder()
        .method("GET")
        .uri("/openapi.yaml")
        .body(axum::body::Body::empty())
        .unwrap();

    let res = app.clone().oneshot(openapi_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let yaml_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert!(yaml_str.contains("https://waymesh.cet.ac.in"));
}
