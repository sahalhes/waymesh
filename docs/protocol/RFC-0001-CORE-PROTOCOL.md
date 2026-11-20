# RFC-0001: Waymesh Core Protocol Specification

- **Status:** Standard Track
- **Version:** 1.1.0
- **Author:** Waymesh Working Group
- **License:** Apache-2.0

## 1. Abstract

Waymesh defines an open protocol for sovereign, peer-to-peer mobility and community logistics coordination. It coordinates passenger carpooling, shared journeys, and parcel crowdshipping without relying on a centralized platform commission or proprietary dispatch monopoly.

## 2. Core Concepts

### 2.1 Nodes
A Waymesh Node is an autonomous service operated by a community, cooperative, student body, or fleet. Nodes store local spatial geometries, execute route-overlap matching, and issue custody verification codes.

### 2.2 Identities
Participants hold Ed25519 keypairs for cryptographic signature verification and authentication. An identity is referenced by its 32-byte public key hex. Nodes never hold private keys; all state mutations are authenticated by Ed25519 signatures.

### 2.3 Journeys
A Journey represents a driver or commuter travelling between Origin and Destination points during a defined departure window. Journeys define a spatial corridor with a buffer radius (default: 1500m), available passenger seats, and cargo weight/volume capacity.

### 2.4 Demands (Rides and Parcels)
- **Passenger Ride:** A request for 1 or more seats from a pickup location to a dropoff location along the corridor.
- **Parcel Delivery:** A request for a peer traveler to transport a package. Requires contents declaration and dimensions.

## 3. Custody & Handover Verification

To eliminate disputes without custodial fiat lockups, Waymesh uses a **Dual-OTP Verifiable Handover State Machine**:

```
[PUBLISHED] 
     │ (Carrier Accepts)
     ▼
[PICKUP_PENDING]  ─── Generates Pickup OTP (known to Sender)
     │                 & Delivery OTP (known to Recipient)
     │ (Physical Meetup: Sender discloses Pickup OTP)
     ▼
[IN_TRANSIT]      ─── Carrier submits Pickup OTP. Legal custody transfers to Carrier.
     │
     │ (Arrival at Destination: Recipient inspects & discloses Delivery OTP)
     ▼
[DELIVERED]       ─── Carrier submits Delivery OTP. Custody released. Compensation settled.
```

## 4. State Transitions

### 4.1 Ride Booking (Supply-Side Driven)
1. `Requested`: Passenger submits pickup/dropoff criteria.
2. `Accepted`: Driver confirms booking; seats are atomically decremented; Pickup OTP is generated for the passenger.
3. `PickupConfirmed`: Driver verifies passenger's OTP at pickup.
4. `Completed`: Driver marks trip completed at destination.
5. `Cancelled`: Either party cancels; reserved seats are atomically restored.

### 4.2 Open Ride Request / Demand (Demand-Side Driven)
When no active matching journeys are currently found along a commuter corridor, a passenger can publish their ride demand directly onto the network:
1. `Published`: Passenger posts pickup/dropoff landmarks, desired departure window, seats required, and optional offered cost share.
2. `Matched`: A commuter driver travelling along the corridor discovers the request (via `/api/v1/ride-requests/search`) and accepts it (via `/api/v1/ride-requests/:id/accept`). An atomic booking is created and the 6-digit pickup OTP is generated.
3. `Cancelled`: Passenger cancels before driver acceptance.
4. `Expired`: The departure time window elapses without fulfillment.

## 5. Settlement & Sound Money Framework

Waymesh is built on real economic value, direct human relationships, and community sovereignty.

### 5.1 Strict Policy: No Speculative Cryptocurrencies / No Blockchain Tokens
Waymesh **explicitly rejects** speculative cryptocurrencies, volatile tokens, Web3 coins, and public blockchain ledgers. 
- Cryptocurrencies introduce extreme speculative volatility, high gas/transaction fees, and artificial complexity that actively harm everyday local transportation.
- Waymesh does **not** issue a coin, does **not** run on a blockchain, and does **not** accept speculative crypto assets.

### 5.2 Private Peer-to-Peer Settlements Between Parties
All travel cost-shares, fuel splits, and delivery compensations are settled directly between parties privately:
- **Private Mutual Settlement:** Participants agree upon cost-sharing directly between each other without platform custody, fee-taking, or intermediation.
- **Direct Exchange & Barter:** For community and rural nodes, direct exchange of local fiat or mutual agreement between peers.

### 5.3 Direct Sovereign Cash Handover
Physical fiat banknotes handed directly between passenger and driver or sender and carrier at the moment of pickup or delivery.
- Completely private, offline-resilient, and zero transaction fees.

### 5.4 Direct Peer-to-Peer National Rails (Zero-Intermediary Digital Fiat)
Instant direct bank transfers using sovereign local rails:
- **UPI (Unified Payments Interface)** in India (instant zero-fee phone-to-phone QR settlement).
- **SEPA Instant / Pix / FedNow** in their respective jurisdictions.
- All payments flow directly between participant bank accounts; nodes never hold, touch, or escrow user funds.

### 5.5 Zero Platform Commission
Nodes operate as community infrastructure. Neither the node operator nor any corporate intermediary extracts a percentage commission from passenger journeys or parcel deliveries.

## 6. Strict Gender Separation Policy for Passenger Rides

To guarantee participant safety, dignity, and cultural alignment across diverse global communities, Waymesh enforces a strict **Gender Separation Rule** for all passenger rides:

### 6.1 The Rule
- **Male and female participants must NEVER be matched together for passenger ride sharing.**
- **Male passengers** may only match with **male drivers** and ride with male passengers.
- **Female passengers** may only match with **female drivers** and ride with female passengers.
- Cross-gender bookings between Male and Female are strictly prohibited across all nodes.

### 6.2 Multi-Layered Enforcement Architecture
1. **Solver Level (Heuristic & Polynomial Solvers):**
   - The `MatchingSolver` candidate generator filters incompatible driver-passenger pairs prior to scoring.
   - Any pairing where `!passenger_gender.is_compatible_for_ride(&driver_gender)` is dropped from candidate evaluation.
2. **API & Protocol Gate Level (`/api/v1/bookings`):**
   - Direct booking requests undergo server-side validation against driver and passenger cryptographic profiles.
   - Incompatible gender pairings are rejected immediately with HTTP `403 Forbidden` (`GenderIncompatible`), preventing race conditions or bypassed solver clients.

### 6.3 Parcel Logistics Exemption
- **Parcels and crowdshipping are gender-agnostic.** Because parcels are inert physical items secured in vehicle cargo/trunk space without passenger cabin co-presence, male carriers may transport parcels for female senders/recipients, and female carriers may transport parcels for male senders/recipients.

## 7. P = NP Future-Proofed Solver Architecture

Waymesh designs for the future where practical polynomial-time algorithms become available for NP-hard routing and combinatorial matching problems (e.g. multi-vehicle pickup and delivery with time windows, quadratic assignment).

- The optimization engine is decoupled behind the `MatchingSolver` async trait.
- Solvers receive immutable `MatchProblem` snapshots and output `SolverSolution` assignments without database locks or protocol mutations.
- Nodes can hot-swap from local geometric spatial corridor heuristics (`SpatialCorridorHeuristic`) to exact integer linear programming (ILP), quantum-annealing simulators, or hypothetical polynomial solvers without altering API schemas or protocol state machines.
