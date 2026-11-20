# Community Roadmap & Contribution Issues (Startup Scaling & Open Source)

Welcome to the Waymesh contributor community! Below are 20 curated, actionable issues designed to scale Waymesh from a solid protocol prototype into a high-scale community mobility startup and vibrant open-source ecosystem.

Whether you are a Rust systems engineer, mobile developer, algorithms researcher, or DevOps specialist, there is a task scoped for you.

To contribute:
1. Find an issue below that matches your interest.
2. Comment on the issue in the [GitHub Issue Tracker](https://github.com/sahalhes/waymesh/issues) to claim it.
3. Follow our [CONTRIBUTING.md](../../CONTRIBUTING.md) to set up your environment, run tests, and open a PR.

---

### [#6](https://github.com/sahalhes/waymesh/issues/6): [Solver] Multi-Hop Parcel Relay Routing for Connecting Travel Corridors
* **Category:** Core Protocol & Algorithms | **Difficulty:** Intermediate
* **Labels:** `enhancement` `help wanted`
* **Problem:** Currently, parcel crowdshipping matches single continuous journeys (e.g., Campus -> Station). When parcels need to travel across non-overlapping corridors (e.g., Trivandrum -> Kollam -> Ernakulam), they cannot be dispatched without a multi-hop relay where Carrier A hands off custody to Carrier B at a connecting hub.
* **Files:** `crates/waymesh-solver/src/lib.rs`, `crates/waymesh-core/src/parcel.rs`, `crates/waymesh-server/src/lib.rs`

**Implementation Steps:**
- Add `RelayWaypoint` struct in `waymesh-core::parcel` defining intermediate meeting points and transfer time windows.
- Formulate multi-hop candidate chaining in `waymesh-solver`: identify intersecting corridors where Carrier 1 ETA at hub <= Carrier 2 departure time at hub - buffer.
- Implement relay custody transfer in `Parcel`: Carrier 1 verifies handover with Hub OTP; Carrier 2 acquires custody with Pickup OTP.
- Add unit tests in `waymesh-solver` covering 2-leg chain matching and time-window feasibility.

**Acceptance Criteria:**
- [ ] Solver successfully returns valid 2-leg assignments for parcels whose origin and destination span two distinct journeys.
- [ ] Dual-OTP state machine handles intermediate relay custody without allowing premature completion.
- [ ] All unit and integration tests pass cleanly with `cargo test --workspace`.

---

### [#7](https://github.com/sahalhes/waymesh/issues/7): [Routing] Pluggable OSRM / Valhalla Engine for Turn-by-Turn Road Detour Scoring
* **Category:** Core Protocol & Algorithms | **Difficulty:** Intermediate
* **Labels:** `enhancement` `help wanted`
* **Problem:** The current `SpatialCorridorHeuristic` uses Haversine geodesic projection for matching. While sub-millisecond fast, it can underestimate detour travel times in regions with winding roads, rivers, or heavy traffic congestion.
* **Files:** `crates/waymesh-solver/src/lib.rs`, `crates/waymesh-server/src/main.rs`, `docker-compose.yml`

**Implementation Steps:**
- Define `#[async_trait] pub trait RoutingEngine: Send + Sync` with `estimate_detour(&self, origin, destination, waypoint) -> Result<DetourEstimate>`.
- Implement default `GeodesicRouter` preserving existing Haversine behavior.
- Implement `OsrmRouter` that queries OSRM table / route HTTP API with local fallback if the endpoint is unreachable.
- Provide an optional `docker-compose.routing.yml` with pre-configured OSRM Kerala/India extract.

**Acceptance Criteria:**
- [ ] Solver can switch between Geodesic and OSRM engine via configuration flag (`WAYMESH_ROUTING_ENGINE=osrm`).
- [ ] Gracefully falls back to geodesic calculation if OSRM HTTP service times out (> 100ms).
- [ ] Zero regression in existing test suite.

---

### [#8](https://github.com/sahalhes/waymesh/issues/8): [Solver] Multi-Stop Sequential Journey Itinerary Optimizer with Detour Penalties
* **Category:** Core Protocol & Algorithms | **Difficulty:** Advanced
* **Labels:** `enhancement` `help wanted`
* **Problem:** When a driver has multiple seats and cargo space, they can pick up several passengers and parcels along the route. Currently, stops are treated independently without optimizing the sequential pickup-and-dropoff visitation order (Travelling Salesperson Problem with pickup/delivery precedence).
* **Files:** `crates/waymesh-solver/src/lib.rs`, `crates/waymesh-core/src/journey.rs`

**Implementation Steps:**
- Define `ItineraryStop` enum (JourneyStart, PassengerPickup, PassengerDropoff, ParcelPickup, ParcelDelivery, JourneyEnd).
- Implement precedence-constrained 2-opt or dynamic programming tour optimizer for itineraries up to 8 stops.
- Enforce that passenger cabin occupancy and trunk cargo weight/volume constraints are never violated at any leg of the tour.
- Add benchmark comparing solver latency before and after multi-stop sequence planning.

**Acceptance Criteria:**
- [ ] Generated itinerary guarantees pickup precedes dropoff for every passenger and parcel.
- [ ] Never exceeds maximum journey detour allowance.
- [ ] Solves 8-stop tour in under 20ms in release mode.

---

### [#9](https://github.com/sahalhes/waymesh/issues/9): [Telemetry] Real-Time Driver Location Stream via SSE with Privacy Corridor Obfuscation
* **Category:** Real-Time Infrastructure | **Difficulty:** Intermediate
* **Labels:** `enhancement` `help wanted`
* **Problem:** Passengers and parcel recipients need live tracking of approaching vehicles. However, streaming exact driver GPS coordinates publicly introduces privacy risks (e.g. exposing residential origins).
* **Files:** `crates/waymesh-server/src/lib.rs`, `crates/waymesh-server/src/db.rs`

**Implementation Steps:**
- Add `POST /api/v1/journeys/:id/telemetry` endpoint for authenticated drivers to post current `{ latitude, longitude, speed, heading }`.
- Add `GET /api/v1/journeys/:id/telemetry` SSE stream returning live telemetry frames.
- Apply differential privacy / coordinate snapping: if requester is not a confirmed passenger on the journey, snap coordinate to corridor centerline and jitter by 200m.
- Add integration test verifying unbooked users receive obfuscated coordinates while confirmed passengers receive exact coordinates.

**Acceptance Criteria:**
- [ ] SSE telemetry stream delivers updates with < 500ms latency.
- [ ] Privacy policy strictly enforced: unauthenticated or unconfirmed users cannot track driver home/origin precision.
- [ ] Clean test coverage in `api_integration_tests.rs`.

---

### [#10](https://github.com/sahalhes/waymesh/issues/10): [Infra] Redis / Dragonfly PubSub Cluster Adapter for Multi-Node Horizontal Scaling
* **Category:** Real-Time Infrastructure | **Difficulty:** Intermediate
* **Labels:** `enhancement` `help wanted`
* **Problem:** `waymesh-server` currently relies on in-memory `tokio::sync::broadcast` channels for events (`/api/v1/events/stream`). Running multiple node replicas behind a load balancer causes event isolation where users connected to Replica B miss events generated on Replica A.
* **Files:** `crates/waymesh-server/Cargo.toml`, `crates/waymesh-server/src/lib.rs`, `docker-compose.yml`

**Implementation Steps:**
- Add `redis = { version = "0.27", features = ["tokio-comp"] }` optional dependency to `crates/waymesh-server`.
- Create an `EventBus` trait with `InMemoryBus` and `RedisPubSubBus` implementations.
- Publish events to Redis channel `waymesh:events` and forward incoming messages to the local Axum SSE streams.
- Add Docker Compose service for testing clustered multi-replica setup.

**Acceptance Criteria:**
- [ ] Node operates in zero-dependency in-memory mode when `REDIS_URL` is absent.
- [ ] Events published on Replica 1 are immediately received by SSE clients on Replica 2 when `REDIS_URL` is set.
- [ ] No memory leaks on reconnect or redis restarts.

---

### [#11](https://github.com/sahalhes/waymesh/issues/11): [Observability] Prometheus Metrics & OpenTelemetry Tracing Endpoint for Node Health
* **Category:** Real-Time Infrastructure | **Difficulty:** Good First Issue
* **Labels:** `enhancement` `good first issue`
* **Problem:** Production operators running Waymesh nodes (universities, fleet co-ops) need metrics for monitoring node throughput, solver latencies, active journeys, OTP verification success/failure rates, and error frequencies.
* **Files:** `crates/waymesh-server/Cargo.toml`, `crates/waymesh-server/src/lib.rs`

**Implementation Steps:**
- Add `metrics` and `metrics-exporter-prometheus` to `waymesh-server` dependencies.
- Instrument key counters and histograms: `waymesh_active_journeys`, `waymesh_active_bookings`, `waymesh_otp_verifications_total{status="success|failed"}`, `waymesh_solver_duration_seconds`.
- Expose `GET /metrics` returning standard Prometheus plaintext format.
- Add automated test verifying `/metrics` responds with valid metric headers and values.

**Acceptance Criteria:**
- [ ] GET `/metrics` returns 200 OK with Prometheus formatted counters and gauges.
- [ ] OTP failure lockout events increment `waymesh_otp_lockouts_total`.
- [ ] Zero impact on core request latency.

---

### [#12](https://github.com/sahalhes/waymesh/issues/12): [Deployment] Turnkey Docker Compose & Kubernetes Helm Chart with Caddy Auto-SSL
* **Category:** DevOps & Infrastructure | **Difficulty:** Good First Issue
* **Labels:** `enhancement` `good first issue` `documentation`
* **Problem:** Community organizations, residential associations, and university clubs want to run their own Waymesh node in minutes without manual server configuration or SSL cert renewal.
* **Files:** `docker-compose.prod.yml`, `Caddyfile`, `deploy/helm/waymesh/Chart.yaml`, `deploy/helm/waymesh/values.yaml`

**Implementation Steps:**
- Create production `docker-compose.prod.yml` with health checks, automatic restart policies, and persistent volumes.
- Configure `Caddyfile` with automatic HTTPS reverse proxying to `waymesh-node:8080`.
- Create basic Helm chart with ConfigMap, Deployment, Service, and Ingress templates.
- Document 1-command startup in `docs/deployment/SELF_HOSTING.md`.

**Acceptance Criteria:**
- [ ] `docker compose -f docker-compose.prod.yml up` starts healthy database, server, and SSL proxy.
- [ ] Server health check passes at `http://localhost:8080/health`.
- [ ] Documentation clearly tested on Ubuntu / Debian VPS.

---

### [#13](https://github.com/sahalhes/waymesh/issues/13): [Safety] Campus & Enterprise Domain Verification Adapter (.edu, .ac.in, corporate)
* **Category:** Trust, Safety & Identity | **Difficulty:** Good First Issue
* **Labels:** `enhancement` `good first issue`
* **Problem:** For women-only carpooling and closed campus communities (e.g. CET, IISc, Technopark), users need a simple way to verify institutional affiliation without leaking personal credentials.
* **Files:** `crates/waymesh-core/src/lib.rs`, `crates/waymesh-server/src/lib.rs`, `crates/waymesh-server/src/db.rs`

**Implementation Steps:**
- Add `verified_domains: Vec<String>` field to identity domain model in `waymesh-core`.
- Implement `POST /api/v1/identities/:id/request-email-verification` with rate limiting.
- Implement `POST /api/v1/identities/:id/confirm-email-verification` validating the 6-digit code.
- Allow filtering journeys and ride requests by required institutional domain.

**Acceptance Criteria:**
- [ ] Users with verified `@cet.ac.in` domain can match with drivers requiring campus verification.
- [ ] Cross-domain or unverified users are filtered out when a journey specifies `required_affiliation`.
- [ ] Full unit test in `api_integration_tests.rs`.

---

### [#14](https://github.com/sahalhes/waymesh/issues/14): [Safety] Cryptographically Signed Ed25519 Peer Vouching & Community Trust Web
* **Category:** Trust, Safety & Identity | **Difficulty:** Intermediate
* **Labels:** `enhancement` `help wanted`
* **Problem:** Centralized rating systems (like 5-star Uber reviews) are susceptible to platform manipulation, bias, and sudden account bans. Waymesh needs a decentralized Web-of-Trust where users cryptographically vouch for peers they have traveled with.
* **Files:** `crates/waymesh-core/src/crypto.rs`, `crates/waymesh-core/src/lib.rs`, `crates/waymesh-server/src/lib.rs`, `crates/waymesh-server/src/db.rs`

**Implementation Steps:**
- Define `PeerVouch` struct with signature verification in `waymesh-core::crypto`.
- Add database table `peer_vouches` storing valid signatures linking passenger and driver.
- Compute an aggregated TrustScore based on verified vouch graph depth.
- Add API endpoint `POST /api/v1/identities/:id/vouch` requiring valid Ed25519 signature.

**Acceptance Criteria:**
- [ ] Invalid signatures or forged vouchers are rejected with cryptographic error.
- [ ] Only parties with a completed `RideBooking` can submit mutual vouches for that trip.
- [ ] Unit tests verify tamper resistance.

---

### [#15](https://github.com/sahalhes/waymesh/issues/15): [Safety] Emergency Corridor Tripwire & Automated Route Deviation Alerting
* **Category:** Trust, Safety & Identity | **Difficulty:** Intermediate
* **Labels:** `enhancement` `help wanted`
* **Problem:** If a vehicle veers significantly off its planned route corridor or stops unexpectedly for an abnormal duration during an active passenger ride, emergency contacts must be alerted immediately.
* **Files:** `crates/waymesh-server/src/lib.rs`, `crates/waymesh-core/src/journey.rs`

**Implementation Steps:**
- Add `EmergencyContact` field to user identities `{ name, phone, email }`.
- Implement periodic tripwire check function in server background worker.
- Compute geodesic distance between current telemetry point and route corridor centerline.
- Emit structured webhook / SSE event `emergency.tripwire_triggered` with GPS coordinates and journey details if boundary breached.

**Acceptance Criteria:**
- [ ] Telemetry points inside the corridor trigger no alerts.
- [ ] Telemetry points exceeding buffer trigger emergency event within 5 seconds.
- [ ] Test scenario simulates deviation and verifies event dispatch.

---

### [#16](https://github.com/sahalhes/waymesh/issues/16): [Settlement] Private Peer-to-Peer Settlement Confirmation Webhook Adapter
* **Category:** Settlement & Private Payments | **Difficulty:** Intermediate
* **Labels:** `enhancement` `help wanted`
* **Problem:** Waymesh mandates 100% peer-to-peer settlement directly between parties privately (via cash, direct peer fiat like UPI, or mutual arrangement) with zero platform custody or commission, and strictly bans speculative cryptocurrency. An automated webhook receiver is needed to verify private settlement confirmations between participants.
* **Files:** `crates/waymesh-server/src/lib.rs`, `crates/waymesh-server/src/db.rs`, `docs/settlement/SETTLEMENT_SPEC.md`

**Implementation Steps:**
- Create `POST /api/v1/settlements/private/webhook` endpoint with HMAC-SHA256 signature verification.
- Match incoming payment reference against `booking_id` or `parcel_id`.
- Record settlement confirmation and transaction timestamp.
- Emit `settlement.completed` event on the node event bus.

**Acceptance Criteria:**
- [ ] Valid webhook updates booking settlement status to `Settled` between parties privately.
- [ ] Unsigned or invalid HMAC webhooks are rejected with 401 Unauthorized.
- [ ] Zero crypto/token dependencies introduced.

---

### [#17](https://github.com/sahalhes/waymesh/issues/17): [Settlement] Dynamic UPI Intent Deep-Link & BharatQR Generation for In-Vehicle Fare Settlement
* **Category:** Settlement & Sound Money | **Difficulty:** Good First Issue
* **Labels:** `enhancement` `good first issue`
* **Problem:** In India, UPI is the ubiquitous zero-fee direct bank rail. Passengers sitting in the car need an instant 1-tap payment link or QR code to transfer agreed fuel cost-shares directly to the driver's bank without third-party fees.
* **Files:** `crates/waymesh-server/src/lib.rs`, `crates/waymesh-server/Cargo.toml`

**Implementation Steps:**
- Add `qrcode` and `image` crates (or standard SVG generator) to `waymesh-server`.
- Build UPI intent URL string format conforming to NPCI UPI specifications.
- Return both raw intent URI and base64 SVG/PNG QR image in endpoint response.
- Add integration test verifying UPI string encoding with INR amount and booking reference.

**Acceptance Criteria:**
- [ ] Generated UPI URL accurately formats driver VPA, passenger name, exact INR cost share, and booking ID.
- [ ] Endpoint returns in < 5ms.
- [ ] Direct peer-to-peer payment with zero platform commission.

---

### [#18](https://github.com/sahalhes/waymesh/issues/18): [Settlement] Mutual Digital Sign-off for Offline Cash Handover
* **Category:** Settlement & Sound Money | **Difficulty:** Good First Issue
* **Labels:** `enhancement` `good first issue`
* **Problem:** Cash remains an essential offline settlement option for rural commuters and students. When physical cash notes are handed over, a mutual digital confirmation prevents fraudulent 'unpaid fare' dispute reports.
* **Files:** `crates/waymesh-core/src/booking.rs`, `crates/waymesh-server/src/lib.rs`

**Implementation Steps:**
- Add `CashSettlementStatus` enum (`PendingHandover`, `ReceivedByDriver`, `ConfirmedByPassenger`) to `RideBooking`.
- Implement `POST /api/v1/bookings/:id/settle-cash` allowing driver to register physical cash receipt.
- Passenger can acknowledge receipt or dispute discrepancy within 2 hours.
- Add unit tests for state transition in `booking.rs`.

**Acceptance Criteria:**
- [ ] State machine prevents false disputes after mutual cash acknowledgement.
- [ ] Booking history logs timestamp and currency notes breakdown if specified.
- [ ] All tests pass.

---

### [#19](https://github.com/sahalhes/waymesh/issues/19): [Client] Lightweight Flutter / React Native Mobile Companion App with Offline OTP Vault
* **Category:** Developer Experience & Clients | **Difficulty:** Help Wanted
* **Labels:** `help wanted` `enhancement`
* **Problem:** While conversational AI interfaces (MCP, ChatGPT Actions) make booking frictionless, drivers and passengers standing at roadside pickup points need an instant, low-bandwidth mobile app for QR codes, offline OTP display, and trip verification.
* **Files:** `apps/mobile/README.md`, `apps/mobile/pubspec.yaml`

**Implementation Steps:**
- Scaffold minimalist mobile client in `apps/mobile`.
- Implement biometric / secure hardware key storage for user Ed25519 identity.
- Build 3 core screens: Corridor Search, Active Trip OTP Vault (large high-contrast digits for sunny outdoors), and Handover Scanner.
- Implement offline caching so active OTP tokens remain visible even with spotty cellular connection.

**Acceptance Criteria:**
- [ ] App launches in < 1.5 seconds.
- [ ] Displays 6-digit pickup and delivery OTPs without active internet connection once trip is accepted.
- [ ] Clean open-source codebase with automated GitHub Actions build.

---

### [#20](https://github.com/sahalhes/waymesh/issues/20): [SDK] TypeScript / JavaScript Client SDK (@waymesh/sdk) with Full OpenAPI Typings
* **Category:** Developer Experience & Clients | **Difficulty:** Good First Issue
* **Labels:** `enhancement` `good first issue` `documentation`
* **Problem:** Frontend web developers, campus portal creators, and bot builders need a zero-dependency, typed TypeScript library to interact with Waymesh nodes without writing manual HTTP requests.
* **Files:** `packages/sdk-ts/package.json`, `packages/sdk-ts/src/index.ts`, `crates/waymesh-server/openapi.yaml`

**Implementation Steps:**
- Set up TypeScript package in `packages/sdk-ts` with `openapi-typescript` and `openapi-fetch`.
- Generate strongly typed client methods for all endpoints (journeys, bookings, parcels, MCP).
- Provide ergonomic helper classes for Ed25519 key generation and OTP verification.
- Write comprehensive README with 5-minute quickstart guide.

**Acceptance Criteria:**
- [ ] `npm run build` generates clean ESM and CJS bundles with full `.d.ts` type declarations.
- [ ] Unit tests execute end-to-end trip creation against mock node in CI.
- [ ] Published ready for npm registry.

---

### [#21](https://github.com/sahalhes/waymesh/issues/21): [CLI] Interactive Terminal CLI (waymesh-cli) for Campus Logistics & Fleet Dispatch
* **Category:** Developer Experience & Clients | **Difficulty:** Good First Issue
* **Labels:** `enhancement` `good first issue`
* **Problem:** System administrators, campus delivery coordinators, and terminal power users need a command-line interface to monitor corridor health, publish fleet journeys, and inspect parcel handovers directly from bash.
* **Files:** `crates/waymesh-cli/Cargo.toml`, `crates/waymesh-cli/src/main.rs`, `Cargo.toml`

**Implementation Steps:**
- Create new workspace crate `crates/waymesh-cli`.
- Implement subcommands with `clap(derive)`: `identity`, `journey`, `booking`, `parcel`, `status`.
- Add colorful terminal table output using `comfy-table`.
- Add `--json` output flag for scripting and piping to `jq`.

**Acceptance Criteria:**
- [ ] `cargo run -p waymesh-cli -- journey list` connects to local node and formats active trips.
- [ ] `--json` produces clean parseable output.
- [ ] Installs cleanly via `cargo install --path crates/waymesh-cli`.

---

### [#22](https://github.com/sahalhes/waymesh/issues/22): [DX] Embedded Scalar / Swagger UI API Explorer at /docs
* **Category:** Developer Experience & Clients | **Difficulty:** Good First Issue
* **Labels:** `enhancement` `good first issue` `documentation`
* **Problem:** Developers exploring a self-hosted Waymesh node currently have to inspect `openapi.yaml` manually. Serving an interactive, beautiful API documentation dashboard at `/docs` drastically improves developer onboarding.
* **Files:** `crates/waymesh-server/src/lib.rs`

**Implementation Steps:**
- Add route `GET /docs` in Axum router.
- Serve standalone Scalar HTML bundle that points to `/openapi.yaml` with modern dark/light mode UI.
- Add navigation link in README and server startup console banner.
- Add test verifying `GET /docs` returns 200 OK with HTML content.

**Acceptance Criteria:**
- [ ] Navigating to `http://localhost:8080/docs` in browser renders interactive Scalar API explorer.
- [ ] All endpoints and schemas can be inspected and tested interactively.
- [ ] Zero runtime external asset dependencies if packaged with inline script.

---

### [#23](https://github.com/sahalhes/waymesh/issues/23): [Testing] High-Concurrency Corridor Traffic Simulation & Benchmark Suite
* **Category:** Testing & Quality Assurance | **Difficulty:** Good First Issue
* **Labels:** `enhancement` `good first issue`
* **Problem:** Before deploying Waymesh to major university campuses (10,000+ daily commuters) or large metropolitan corridors, the community needs automated load testing scripts measuring match throughput and database connection pooling under heavy concurrent load.
* **Files:** `benchmarks/load/corridor_load_test.js`, `benchmarks/load/README.md`

**Implementation Steps:**
- Write k6 load test script generating realistic geo-coordinates along realistic corridors.
- Measure p95 and p99 response times for `/api/v1/matches/search` and `/api/v1/journeys`.
- Output performance summary table with requests/sec and memory footprint.
- Add GitHub Actions benchmark trigger for performance regression tracking.

**Acceptance Criteria:**
- [ ] Test simulates 1,000 concurrent virtual users sustained over 2 minutes.
- [ ] Maintains p95 response time under 50ms for in-memory matching.
- [ ] Clear instructions in `benchmarks/load/README.md`.

---

### [#24](https://github.com/sahalhes/waymesh/issues/24): [Protocol] Inter-Node Corridor Gossip & Peer Federated Discovery (Phase 2 RFC)
* **Category:** Decentralization & Protocol Scaling | **Difficulty:** Advanced
* **Labels:** `enhancement` `help wanted`
* **Problem:** Waymesh is architected as an autonomous community protocol. Campus nodes (e.g. CET Trivandrum) and City nodes (e.g. Kochi Metro Co-op) need to peer with each other to exchange inter-city corridor trajectories without central authority.
* **Files:** `docs/rfc/RFC_0002_FEDERATED_NODE_GOSSIP.md`, `crates/waymesh-core/src/lib.rs`

**Implementation Steps:**
- Write comprehensive RFC document defining node handshake, signed peer discovery, and privacy preservation.
- Specify inter-node message wire format over HTTPS or libp2p.
- Prototype cross-node search proxying: Node A queries Node B when corridor crosses node bounding box.
- Review threat model (Sybil attacks, corridor spamming).

**Acceptance Criteria:**
- [ ] Complete RFC specification document approved in `docs/rfc/`.
- [ ] Initial proof-of-concept wire test demonstrating federated query dispatch between 2 mock nodes.
- [ ] Adheres to zero-blockchain, pure cryptographic trust principles.

---

### [#25](https://github.com/sahalhes/waymesh/issues/25): [Security] Automated Cryptographic Audit & Memory Zeroization Hardening
* **Category:** Trust, Safety & Identity | **Difficulty:** Intermediate
* **Labels:** `enhancement` `help wanted`
* **Problem:** To ensure enterprise-grade security for user cryptographic identities, secret keys in memory must be zeroized upon drop, and all hash comparisons must strictly use constant-time operations to prevent side-channel timing attacks.
* **Files:** `crates/waymesh-core/Cargo.toml`, `crates/waymesh-core/src/crypto.rs`, `crates/waymesh-core/src/booking.rs`, `crates/waymesh-core/src/parcel.rs`

**Implementation Steps:**
- Add `zeroize = { version = "1.8", features = ["derive"] }` and `subtle = "2.6"` to `waymesh-core`.
- Implement `Zeroize` and `ZeroizeOnDrop` for internal keypair memory structures.
- Replace standard string equality in OTP verification with `subtle::ConstantTimeEq`.
- Add security regression test verifying memory zeroization.

**Acceptance Criteria:**
- [ ] All OTP comparisons execute in constant time regardless of character match position.
- [ ] Signing keys zeroize secret bytes on drop.
- [ ] Passes `cargo clippy` with zero warnings.

---

