use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let node_url =
        std::env::var("WAYMESH_NODE_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into());
    let client = reqwest::Client::new();

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        let req: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let err_res = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": format!("Parse error: {}", e) }
                });
                writeln!(stdout, "{}", err_res)?;
                stdout.flush()?;
                continue;
            }
        };

        let response = handle_mcp_method(&client, &node_url, &req).await;
        if let Some(resp) = response {
            writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
            stdout.flush()?;
        }
    }

    Ok(())
}

async fn handle_mcp_method(
    client: &reqwest::Client,
    node_url: &str,
    req: &JsonRpcRequest,
) -> Option<Value> {
    let req_id = req.id.clone().unwrap_or(Value::Null);

    match req.method.as_str() {
        "initialize" => Some(json!({
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
                }
            }
        })),
        "notifications/initialized" => None,
        "tools/list" => Some(json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {
                "tools": [
                    {
                        "name": "waymesh_register_identity",
                        "description": "Register a new user, driver, or carrier cryptographic identity on the Waymesh node with Ed25519 public key and gender.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "public_key_ed25519": { "type": "string", "description": "64-character hex Ed25519 public key" },
                                "display_name": { "type": "string", "description": "Display name or alias" },
                                "gender": { "type": "string", "enum": ["male", "female", "other"], "description": "Gender for ride safety matching (male and female are never matched together in passenger rides)" }
                            },
                            "required": ["public_key_ed25519", "display_name"]
                        }
                    },
                    {
                        "name": "waymesh_publish_journey",
                        "description": "Publish a driver or traveler journey with spare seats or cargo capacity along a corridor. Automatically checks for matching passenger ride requests waiting along the corridor.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "driver_id": { "type": "string", "description": "Optional UUID of driver identity" },
                                "driver_name": { "type": "string", "description": "Display name of driver (e.g. 'Sunita Rao')" },
                                "driver_gender": { "type": "string", "enum": ["male", "female", "other"], "description": "Driver gender for gender compatibility matching (strict separation between male and female in passenger rides)" },
                                "origin_name": { "type": "string" },
                                "origin_lat": { "type": "number" },
                                "origin_lng": { "type": "number" },
                                "destination_name": { "type": "string" },
                                "destination_lat": { "type": "number" },
                                "destination_lng": { "type": "number" },
                                "departure_time": { "type": "string", "format": "date-time" },
                                "seats": { "type": "integer", "default": 3 },
                                "cargo_kg": { "type": "number", "default": 10.0 },
                                "cost_per_seat": { "type": "integer", "description": "Cost per seat in minor units (e.g. 8000 = ₹80.00)" },
                                "suggested_cost_per_seat": { "type": "integer", "description": "Cost per seat in minor units (e.g. 8000 = ₹80.00)" }
                            },
                            "required": ["origin_name", "destination_name", "departure_time"]
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
                                "origin_lat": { "type": "number", "description": "Driver departure latitude (auto-resolved if 0)" },
                                "origin_lng": { "type": "number", "description": "Driver departure longitude (auto-resolved if 0)" },
                                "destination_name": { "type": "string", "description": "Optional driver destination landmark (e.g. 'TVM Central Railway Station')" },
                                "destination_lat": { "type": "number", "description": "Driver destination latitude (auto-resolved if 0)" },
                                "destination_lng": { "type": "number", "description": "Driver destination longitude (auto-resolved if 0)" },
                                "departure_time": { "type": "string", "format": "date-time", "description": "Driver departure time" },
                                "driver_gender": { "type": "string", "enum": ["male", "female", "other"], "description": "Driver gender (strictly matches compatible passengers)" },
                                "max_detour_minutes": { "type": "integer", "default": 15 }
                            }
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
                        "name": "waymesh_search_matches",
                        "description": "Search available rides or parcel carriers along a corridor. Strict gender matching policy: male and female are never matched together for passenger rides.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "pickup_lat": { "type": "number" },
                                "pickup_lng": { "type": "number" },
                                "dropoff_lat": { "type": "number" },
                                "dropoff_lng": { "type": "number" },
                                "desired_time": { "type": "string", "format": "date-time" },
                                "demand_type": { "type": "string", "enum": ["passenger", "parcel"] },
                                "seats": { "type": "integer", "default": 1 },
                                "cargo_kg": { "type": "number", "default": 0.0 },
                                "passenger_id": { "type": "string", "description": "Optional UUID of passenger to lookup profile gender" },
                                "passenger_gender": { "type": "string", "enum": ["male", "female", "other"], "description": "Passenger gender (male and female are never matched together for passenger rides)" }
                            },
                            "required": ["pickup_lat", "pickup_lng", "dropoff_lat", "dropoff_lng", "desired_time", "demand_type"]
                        }
                    },
                    {
                        "name": "waymesh_create_booking",
                        "description": "Request a seat on a matched journey. Zero-UUID support: passenger_id is optional; if omitted, identity is automatically created using passenger_name. Strictly enforces gender separation (returns 403 Forbidden if passenger and driver genders conflict).",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "journey_id": { "type": "string" },
                                "passenger_id": { "type": "string", "description": "Optional UUID of passenger. Omit to auto-provision." },
                                "passenger_name": { "type": "string", "description": "Display name of passenger (e.g. 'Priya Nair', 'CET Student')" },
                                "passenger_gender": { "type": "string", "enum": ["male", "female", "other"], "description": "Passenger gender (strictly checked against driver gender; male and female pairings are forbidden)" },
                                "seats": { "type": "integer", "default": 1 },
                                "pickup_name": { "type": "string" },
                                "pickup_lat": { "type": "number" },
                                "pickup_lng": { "type": "number" },
                                "dropoff_name": { "type": "string" },
                                "dropoff_lat": { "type": "number" },
                                "dropoff_lng": { "type": "number" },
                                "agreed_cost_share": { "type": "integer" }
                            },
                            "required": ["journey_id", "seats", "pickup_name", "pickup_lat", "pickup_lng", "dropoff_name", "dropoff_lat", "dropoff_lng", "agreed_cost_share"]
                        }
                    },
                    {
                        "name": "waymesh_accept_booking",
                        "description": "Driver accepts a pending ride booking request, generating the passenger's pickup OTP.",
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
                        "description": "Request a peer traveler to transport a package along their journey. Zero-UUID and landmark support: sender_id and coordinates are optional; landmarks auto-resolve.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "sender_id": { "type": "string", "description": "Optional UUID of sender identity" },
                                "sender_name": { "type": "string", "description": "Display name of sender (e.g. 'Anjali Sharma')" },
                                "recipient_name": { "type": "string" },
                                "recipient_phone": { "type": "string" },
                                "pickup_address": { "type": "string", "description": "Pickup address or landmark (e.g. 'Technopark Phase 1')" },
                                "pickup_lat": { "type": "number", "default": 0.0 },
                                "pickup_lng": { "type": "number", "default": 0.0 },
                                "dropoff_address": { "type": "string", "description": "Delivery address or landmark (e.g. 'Thampanoor')" },
                                "dropoff_lat": { "type": "number", "default": 0.0 },
                                "dropoff_lng": { "type": "number", "default": 0.0 },
                                "weight_kg": { "type": "number" },
                                "volume_liters": { "type": "number", "default": 5.0 },
                                "contents": { "type": "string", "description": "Description of package contents" },
                                "deadline_time": { "type": "string", "description": "Deadline (e.g. 'today 5 pm' or ISO-8601)" },
                                "compensation": { "type": "integer", "description": "Compensation in paise (e.g. 15000 = ₹150)" }
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
                        "description": "Verify physical handover of a ride or parcel using the 6-digit OTP code.",
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
                    }
                ]
            }
        })),
        "tools/call" => {
            let params = req.params.as_ref()?;
            let tool_name = params.get("name")?.as_str()?;
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

            let result = execute_tool(client, node_url, tool_name, arguments).await;
            match result {
                Ok(val) => Some(json!({
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": {
                        "content": [
                            {
                                "type": "text",
                                "text": serde_json::to_string_pretty(&val).unwrap_or_default()
                            }
                        ]
                    }
                })),
                Err(err_msg) => Some(json!({
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": {
                        "isError": true,
                        "content": [
                            {
                                "type": "text",
                                "text": format!("Tool execution failed: {}", err_msg)
                            }
                        ]
                    }
                })),
            }
        }
        _ => Some(json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "error": { "code": -32601, "message": format!("Method not found: {}", req.method) }
        })),
    }
}

async fn handle_resp(res: reqwest::Response) -> Result<Value, String> {
    let status = res.status();
    let text = res.text().await.map_err(|e| e.to_string())?;
    if let Ok(json_val) = serde_json::from_str::<Value>(&text) {
        if status.is_success() {
            Ok(json_val)
        } else {
            let msg = json_val
                .get("error")
                .or_else(|| json_val.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or(&text);
            Err(format!(
                "HTTP {} {}: {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or("Error"),
                msg
            ))
        }
    } else {
        if status.is_success() {
            Ok(serde_json::json!({ "message": text }))
        } else {
            Err(format!(
                "HTTP {} {}: {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or("Error"),
                text
            ))
        }
    }
}

async fn execute_tool(
    client: &reqwest::Client,
    node_url: &str,
    name: &str,
    mut args: Value,
) -> Result<Value, String> {
    if let Some(cost) = args.get("cost_per_seat").and_then(|v| v.as_u64()) {
        if args.get("suggested_cost_per_seat").is_none() {
            args["suggested_cost_per_seat"] = json!(cost);
        }
    }

    match name {
        "waymesh_register_identity" => {
            let res = client
                .post(format!("{}/api/v1/identities", node_url))
                .json(&args)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            handle_resp(res).await
        }
        "waymesh_publish_journey" => {
            let res = client
                .post(format!("{}/api/v1/journeys", node_url))
                .json(&args)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            handle_resp(res).await
        }
        "waymesh_publish_ride_request" => {
            let res = client
                .post(format!("{}/api/v1/ride-requests", node_url))
                .json(&args)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            handle_resp(res).await
        }
        "waymesh_search_ride_requests" => {
            let res = client
                .post(format!("{}/api/v1/ride-requests/search", node_url))
                .json(&args)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            handle_resp(res).await
        }
        "waymesh_accept_ride_request" => {
            let request_id = args
                .get("request_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing request_id")?;
            let res = client
                .post(format!(
                    "{}/api/v1/ride-requests/{}/accept",
                    node_url, request_id
                ))
                .json(&args)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            handle_resp(res).await
        }
        "waymesh_get_ride_request_status" => {
            let request_id = args
                .get("request_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing request_id")?;
            let res = client
                .get(format!("{}/api/v1/ride-requests/{}", node_url, request_id))
                .send()
                .await
                .map_err(|e| e.to_string())?;
            handle_resp(res).await
        }
        "waymesh_cancel_ride_request" => {
            let request_id = args
                .get("request_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing request_id")?;
            let res = client
                .post(format!(
                    "{}/api/v1/ride-requests/{}/cancel",
                    node_url, request_id
                ))
                .send()
                .await
                .map_err(|e| e.to_string())?;
            handle_resp(res).await
        }
        "waymesh_search_matches" => {
            let res = client
                .post(format!("{}/api/v1/matches/search", node_url))
                .json(&args)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            handle_resp(res).await
        }
        "waymesh_create_booking" => {
            if args.get("pickup_lat").is_none() {
                args["pickup_lat"] = json!(0.0);
            }
            if args.get("pickup_lng").is_none() {
                args["pickup_lng"] = json!(0.0);
            }
            if args.get("dropoff_lat").is_none() {
                args["dropoff_lat"] = json!(0.0);
            }
            if args.get("dropoff_lng").is_none() {
                args["dropoff_lng"] = json!(0.0);
            }
            let res = client
                .post(format!("{}/api/v1/bookings", node_url))
                .json(&args)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            handle_resp(res).await
        }
        "waymesh_accept_booking" => {
            let booking_id = args
                .get("booking_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing booking_id")?;
            let res = client
                .post(format!(
                    "{}/api/v1/bookings/{}/accept",
                    node_url, booking_id
                ))
                .send()
                .await
                .map_err(|e| e.to_string())?;
            handle_resp(res).await
        }
        "waymesh_request_parcel_delivery" => {
            if args.get("pickup_lat").is_none() {
                args["pickup_lat"] = json!(0.0);
            }
            if args.get("pickup_lng").is_none() {
                args["pickup_lng"] = json!(0.0);
            }
            if args.get("dropoff_lat").is_none() {
                args["dropoff_lat"] = json!(0.0);
            }
            if args.get("dropoff_lng").is_none() {
                args["dropoff_lng"] = json!(0.0);
            }
            let res = client
                .post(format!("{}/api/v1/parcels", node_url))
                .json(&args)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            handle_resp(res).await
        }
        "waymesh_accept_parcel" => {
            let parcel_id = args
                .get("parcel_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing parcel_id")?;
            let res = client
                .post(format!("{}/api/v1/parcels/{}/accept", node_url, parcel_id))
                .json(&args)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            handle_resp(res).await
        }
        "waymesh_verify_handover" => {
            let entity_type = args
                .get("entity_type")
                .and_then(|v| v.as_str())
                .unwrap_or("booking");
            let entity_id = args
                .get("entity_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing entity_id")?;
            let otp = args
                .get("otp")
                .and_then(|v| v.as_str())
                .ok_or("Missing otp")?;
            let action = args
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("pickup");

            let url = if entity_type == "parcel" {
                if action == "deliver" {
                    format!("{}/api/v1/parcels/{}/deliver", node_url, entity_id)
                } else {
                    format!("{}/api/v1/parcels/{}/pickup", node_url, entity_id)
                }
            } else {
                format!("{}/api/v1/bookings/{}/verify-pickup", node_url, entity_id)
            };

            let res = client
                .post(&url)
                .json(&json!({ "otp": otp }))
                .send()
                .await
                .map_err(|e| e.to_string())?;
            handle_resp(res).await
        }
        "waymesh_cancel_booking" => {
            let booking_id = args
                .get("booking_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing booking_id")?;
            let res = client
                .post(format!(
                    "{}/api/v1/bookings/{}/cancel",
                    node_url, booking_id
                ))
                .send()
                .await
                .map_err(|e| e.to_string())?;
            handle_resp(res).await
        }
        "waymesh_cancel_journey" => {
            let journey_id = args
                .get("journey_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing journey_id")?;
            let res = client
                .post(format!(
                    "{}/api/v1/journeys/{}/cancel",
                    node_url, journey_id
                ))
                .send()
                .await
                .map_err(|e| e.to_string())?;
            handle_resp(res).await
        }
        "waymesh_cancel_parcel" => {
            let parcel_id = args
                .get("parcel_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing parcel_id")?;
            let res = client
                .post(format!("{}/api/v1/parcels/{}/cancel", node_url, parcel_id))
                .send()
                .await
                .map_err(|e| e.to_string())?;
            handle_resp(res).await
        }
        _ => Err(format!("Unknown tool: {}", name)),
    }
}
