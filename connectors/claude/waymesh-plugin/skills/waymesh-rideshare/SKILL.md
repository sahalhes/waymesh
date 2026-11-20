---
name: waymesh-rideshare
description: Search, match, book, or publish peer-to-peer rides, safe carpooling, and crowdshipping logistics on the Waymesh network.
when_to_use: When the user asks for a ride, carpool, shared auto, parcel delivery, wants to publish travel corridor capacity, verify an OTP handover, or mentions Waymesh.
---

# Waymesh Mobility & Logistics Skill

Use this skill whenever the user wants to:
- Find or book a shared ride, auto, or carpool.
- Find safe carpooling (strict same-gender ride matching).
- Publish a driver journey with empty seats or cargo trunk space.
- Request a peer traveler to carry a parcel along their route.
- Verify a pickup or delivery using a 6-digit OTP code.

## Critical Rules & Protocol Workflow

### 1. Strict Gender Separation (Safe Carpooling Protocol)
- **Male and female riders must NEVER be matched together for passenger rides.**
- Only male connects with male, and female connects with female for carpooling.
- If a female passenger searches for a ride, set `passenger_gender: "female"` (matches strictly with female drivers).
- If a male passenger searches for a ride, set `passenger_gender: "male"` (matches strictly with male drivers).
- If passenger gender is unspecified, politely ask before confirming a booking: *"Waymesh guarantees safe same-gender carpooling. Are you booking for a female or male commuter?"*
- Parcel crowdshipping is gender-agnostic (cargo is kept in the vehicle trunk/boot without passenger cabin presence).

### 2. Zero-UUID Conversational UX
- **NEVER** ask users for raw technical UUIDs, Ed25519 public keys, or database hashes.
- When calling `waymesh_create_booking`, pass `passenger_name` (e.g. "CET Student", "Priya") and `passenger_gender`. The node automatically provisions and binds the cryptographic identity in the background.
- When publishing a journey, pass `driver_name` and `driver_gender`.
- Retain returned `journey_id`, `booking_id`, and `parcel_id` in session memory to chain subsequent calls (e.g. accept or verify handover).

### 3. Automatic Landmark Coordinate Resolution
Resolve local landmarks automatically to approximate coordinates:
- **Chavadimukku / Chavadimuk (Trivandrum):** Latitude `8.5372`, Longitude `76.9048`
- **CET (College of Engineering Trivandrum), Sreekaryam:** Latitude `8.5458`, Longitude `76.9063`
- **Kochuveli Railway Station (KCVL):** Latitude `8.5085`, Longitude `76.9038`
- **Trivandrum Central Railway Station (TVC / Thampanoor):** Latitude `8.4875`, Longitude `76.9525`
- **Technopark Phase 1 / Phase 3 (Trivandrum):** Latitude `8.5580`, Longitude `76.8770`
- **Campus North Gate (Bengaluru):** Latitude `12.9716`, Longitude `77.5946`
- **City Central Railway Station (Bengaluru):** Latitude `12.9780`, Longitude `77.5700`

> **Station Disambiguation in Trivandrum**:
> - **Kochuveli (KCVL)** is located at `8.5085, 76.9038` on the coastal railway corridor.
> - **Trivandrum Central (TVC / Thampanoor)** is located at `8.4875, 76.9525` in the city center (~7 km south).
> - Never confuse or conflate TVC and KCVL. When a user asks for KCVL, search/publish specifically for Kochuveli.

### 4. Anti-Browse Mandate (Never Search the Web for Rides)
- **STRICTLY PROHIBITED**: Never invoke external web search, Google, Bing, or browser tools when a user searches for a ride, carpool, or delivery.
- Waymesh is a real-time peer-to-peer network, not a public web directory. Web search leaks user travel intent, returns irrelevant results, and degrades user privacy.
- If `waymesh_search_matches` returns 0 active journeys, **immediately publish the commuter's open ride request** using `waymesh_publish_ride_request`.

### 5. Verifiable Handover with 6-Digit OTP
- When a driver accepts a ride booking, a secret 6-digit Pickup OTP is generated.
- Always display this 6-digit code clearly to the passenger (e.g. `🔑 Your Secret Pickup OTP: [123456]`).
- Explain that the passenger discloses this OTP to the driver upon physical meeting to verify boarding cryptographically.
- For parcels, both Pickup OTP (disclosed by sender to carrier) and Delivery OTP (disclosed by recipient to carrier) are used.

### 6. Private Peer-to-Peer Settlement (No Crypto / No Web3)
- Settle fares directly between parties privately in Indian Rupees (₹), direct peer UPI QR/handles, offline physical cash, or mutual private agreement.
- Strictly zero speculative cryptocurrency, zero Web3 tokens, zero blockchain gas.

### 7. Bidirectional Commute & Ride Requests ("Check Ride or Offer if Not Present")
- **Anyone can check if a ride is there, or offer a ride if not present:**
  - **If User is a Passenger Looking for a Ride**:
    1. Search for available rides along the corridor: `waymesh_search_matches`.
    2. If a matching ride is found, book via `waymesh_create_booking`.
    3. **If NO matching ride is found (or if the user says "just publish", "post request", "find me one when available")**:
       - **Immediately publish a passenger ride request using `waymesh_publish_ride_request`!**
       - Pass `origin_name`, `destination_name`, `desired_time`, `passenger_gender` ("male" or "female"), and `passenger_name`.
       - Inform the user that their ride request is published and visible to all verified same-gender drivers traveling along that corridor.
  - **If User is a Driver Offering a Ride**:
    1. Check for passengers requesting rides along their route: `waymesh_search_ride_requests` (can pass landmark names `origin_name` and `destination_name`).
    2. If a passenger request is found, accept it directly via `waymesh_accept_ride_request`.
    3. Or publish their journey corridor with empty seats: `waymesh_publish_journey`. The node automatically notifies of any matching passenger requests along the route.
  - **Checking Status & Retrieving OTP**:
    - When a passenger asks *"Has anyone accepted my ride?"* or *"What is my pickup OTP?"*, call `waymesh_get_ride_request_status` with `request_id`.
    - If accepted, present the driver details and secret 6-digit pickup OTP.
  - **Cancelling Ride Requests**:
    - If a passenger says *"Cancel my ride"* or changed plans, call `waymesh_cancel_ride_request` with `request_id`.

## MCP Tools Available

The plugin connects directly to the Waymesh node via Model Context Protocol:
- `waymesh_search_matches`: Search available rides or parcel carriers along a spatial corridor.
- `waymesh_publish_ride_request`: Publish a passenger ride request (commuter demand / wanted ride) when no ride is currently available, or when passenger asks to publish.
- `waymesh_search_ride_requests`: Search open passenger ride requests along a corridor for drivers looking to offer a ride. Supports landmark names.
- `waymesh_accept_ride_request`: Driver accepts/fulfills an open passenger ride request, generating the secret 6-digit pickup OTP.
- `waymesh_get_ride_request_status`: Check status of a published ride request (returns driver details, booking ID, and pickup OTP if matched).
- `waymesh_cancel_ride_request`: Cancel an active published ride request.
- `waymesh_publish_journey`: Publish a planned driver or commuter journey with route coordinates, empty seats, and cargo capacity.
- `waymesh_create_booking`: Reserve a seat on a matched journey with automatic identity provisioning.
- `waymesh_accept_booking`: Driver accepts a booking, generating the secret 6-digit pickup OTP.
- `waymesh_request_parcel_delivery`: Publish a parcel delivery request.
- `waymesh_accept_parcel`: Carrier accepts a parcel delivery request.
- `waymesh_verify_handover`: Verify passenger pickup or parcel custody transfer with the 6-digit cryptographic OTP.
