#!/usr/bin/env python3
import json
import os
import subprocess
import sys
import time
import urllib.request
import urllib.error

# 1. Retrieve GitHub Token from git credential helper
def get_github_token():
    try:
        proc = subprocess.Popen(
            ["git", "credential", "fill"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True
        )
        out, _ = proc.communicate("protocol=https\nhost=github.com\n")
        for line in out.splitlines():
            if line.startswith("password="):
                return line.split("=", 1)[1].strip()
    except Exception as e:
        print(f"Error getting credential: {e}")
    return os.environ.get("GITHUB_TOKEN")

REPO = "sahalhes/waymesh"

ISSUES = [
    {
        "title": "[Solver] Multi-Hop Parcel Relay Routing for Connecting Travel Corridors",
        "labels": ["enhancement", "help wanted"],
        "category": "Core Protocol & Algorithms",
        "difficulty": "Intermediate",
        "problem": "Currently, parcel crowdshipping matches single continuous journeys (e.g., Campus -> Station). When parcels need to travel across non-overlapping corridors (e.g., Trivandrum -> Kollam -> Ernakulam), they cannot be dispatched without a multi-hop relay where Carrier A hands off custody to Carrier B at a connecting hub.",
        "proposal": "Extend `waymesh-solver` to formulate 2-leg and 3-leg transfer chains with intermediate waypoints. Extend the Dual-OTP handover state machine in `waymesh-core::parcel` to generate intermediate transit verification tokens for hub exchanges.",
        "affected_files": [
            "crates/waymesh-solver/src/lib.rs",
            "crates/waymesh-core/src/parcel.rs",
            "crates/waymesh-server/src/lib.rs"
        ],
        "steps": [
            "Add `RelayWaypoint` struct in `waymesh-core::parcel` defining intermediate meeting points and transfer time windows.",
            "Formulate multi-hop candidate chaining in `waymesh-solver`: identify intersecting corridors where Carrier 1 ETA at hub <= Carrier 2 departure time at hub - buffer.",
            "Implement relay custody transfer in `Parcel`: Carrier 1 verifies handover with Hub OTP; Carrier 2 acquires custody with Pickup OTP.",
            "Add unit tests in `waymesh-solver` covering 2-leg chain matching and time-window feasibility."
        ],
        "acceptance_criteria": [
            "Solver successfully returns valid 2-leg assignments for parcels whose origin and destination span two distinct journeys.",
            "Dual-OTP state machine handles intermediate relay custody without allowing premature completion.",
            "All unit and integration tests pass cleanly with `cargo test --workspace`."
        ]
    },
    {
        "title": "[Routing] Pluggable OSRM / Valhalla Engine for Turn-by-Turn Road Detour Scoring",
        "labels": ["enhancement", "help wanted"],
        "category": "Core Protocol & Algorithms",
        "difficulty": "Intermediate",
        "problem": "The current `SpatialCorridorHeuristic` uses Haversine geodesic projection for matching. While sub-millisecond fast, it can underestimate detour travel times in regions with winding roads, rivers, or heavy traffic congestion.",
        "proposal": "Introduce a pluggable `RoutingEngine` trait in `waymesh-solver` with two implementations: `GeodesicRouter` (current default, zero-dependency) and `OsrmRouter` / `ValhallaRouter` (HTTP client querying local OpenStreetMap routing instances).",
        "affected_files": [
            "crates/waymesh-solver/src/lib.rs",
            "crates/waymesh-server/src/main.rs",
            "docker-compose.yml"
        ],
        "steps": [
            "Define `#[async_trait] pub trait RoutingEngine: Send + Sync` with `estimate_detour(&self, origin, destination, waypoint) -> Result<DetourEstimate>`.",
            "Implement default `GeodesicRouter` preserving existing Haversine behavior.",
            "Implement `OsrmRouter` that queries OSRM table / route HTTP API with local fallback if the endpoint is unreachable.",
            "Provide an optional `docker-compose.routing.yml` with pre-configured OSRM Kerala/India extract."
        ],
        "acceptance_criteria": [
            "Solver can switch between Geodesic and OSRM engine via configuration flag (`WAYMESH_ROUTING_ENGINE=osrm`).",
            "Gracefully falls back to geodesic calculation if OSRM HTTP service times out (> 100ms).",
            "Zero regression in existing test suite."
        ]
    },
    {
        "title": "[Solver] Multi-Stop Sequential Journey Itinerary Optimizer with Detour Penalties",
        "labels": ["enhancement", "help wanted"],
        "category": "Core Protocol & Algorithms",
        "difficulty": "Advanced",
        "problem": "When a driver has multiple seats and cargo space, they can pick up several passengers and parcels along the route. Currently, stops are treated independently without optimizing the sequential pickup-and-dropoff visitation order (Travelling Salesperson Problem with pickup/delivery precedence).",
        "proposal": "Implement a lightweight topological tour planner in `waymesh-solver` that ensures for every demand, `pickup_time < dropoff_time`, while minimizing total additional detour duration.",
        "affected_files": [
            "crates/waymesh-solver/src/lib.rs",
            "crates/waymesh-core/src/journey.rs"
        ],
        "steps": [
            "Define `ItineraryStop` enum (JourneyStart, PassengerPickup, PassengerDropoff, ParcelPickup, ParcelDelivery, JourneyEnd).",
            "Implement precedence-constrained 2-opt or dynamic programming tour optimizer for itineraries up to 8 stops.",
            "Enforce that passenger cabin occupancy and trunk cargo weight/volume constraints are never violated at any leg of the tour.",
            "Add benchmark comparing solver latency before and after multi-stop sequence planning."
        ],
        "acceptance_criteria": [
            "Generated itinerary guarantees pickup precedes dropoff for every passenger and parcel.",
            "Never exceeds maximum journey detour allowance.",
            "Solves 8-stop tour in under 20ms in release mode."
        ]
    },
    {
        "title": "[Telemetry] Real-Time Driver Location Stream via SSE with Privacy Corridor Obfuscation",
        "labels": ["enhancement", "help wanted"],
        "category": "Real-Time Infrastructure",
        "difficulty": "Intermediate",
        "problem": "Passengers and parcel recipients need live tracking of approaching vehicles. However, streaming exact driver GPS coordinates publicly introduces privacy risks (e.g. exposing residential origins).",
        "proposal": "Create a real-time Server-Sent Events (SSE) telemetry endpoint (`/api/v1/journeys/:id/telemetry`) that streams driver coordinates. Obfuscate driver locations by snapping them to the travel corridor or rounding coordinates until a booking is accepted and pickup is within 15 minutes.",
        "affected_files": [
            "crates/waymesh-server/src/lib.rs",
            "crates/waymesh-server/src/db.rs"
        ],
        "steps": [
            "Add `POST /api/v1/journeys/:id/telemetry` endpoint for authenticated drivers to post current `{ latitude, longitude, speed, heading }`.",
            "Add `GET /api/v1/journeys/:id/telemetry` SSE stream returning live telemetry frames.",
            "Apply differential privacy / coordinate snapping: if requester is not a confirmed passenger on the journey, snap coordinate to corridor centerline and jitter by 200m.",
            "Add integration test verifying unbooked users receive obfuscated coordinates while confirmed passengers receive exact coordinates."
        ],
        "acceptance_criteria": [
            "SSE telemetry stream delivers updates with < 500ms latency.",
            "Privacy policy strictly enforced: unauthenticated or unconfirmed users cannot track driver home/origin precision.",
            "Clean test coverage in `api_integration_tests.rs`."
        ]
    },
    {
        "title": "[Infra] Redis / Dragonfly PubSub Cluster Adapter for Multi-Node Horizontal Scaling",
        "labels": ["enhancement", "help wanted"],
        "category": "Real-Time Infrastructure",
        "difficulty": "Intermediate",
        "problem": "`waymesh-server` currently relies on in-memory `tokio::sync::broadcast` channels for events (`/api/v1/events/stream`). Running multiple node replicas behind a load balancer causes event isolation where users connected to Replica B miss events generated on Replica A.",
        "proposal": "Introduce an optional Redis/Dragonfly PubSub backend for the event bus. If `REDIS_URL` is set, broadcast events across all server instances.",
        "affected_files": [
            "crates/waymesh-server/Cargo.toml",
            "crates/waymesh-server/src/lib.rs",
            "docker-compose.yml"
        ],
        "steps": [
            "Add `redis = { version = \"0.27\", features = [\"tokio-comp\"] }` optional dependency to `crates/waymesh-server`.",
            "Create an `EventBus` trait with `InMemoryBus` and `RedisPubSubBus` implementations.",
            "Publish events to Redis channel `waymesh:events` and forward incoming messages to the local Axum SSE streams.",
            "Add Docker Compose service for testing clustered multi-replica setup."
        ],
        "acceptance_criteria": [
            "Node operates in zero-dependency in-memory mode when `REDIS_URL` is absent.",
            "Events published on Replica 1 are immediately received by SSE clients on Replica 2 when `REDIS_URL` is set.",
            "No memory leaks on reconnect or redis restarts."
        ]
    },
    {
        "title": "[Observability] Prometheus Metrics & OpenTelemetry Tracing Endpoint for Node Health",
        "labels": ["enhancement", "good first issue"],
        "category": "Real-Time Infrastructure",
        "difficulty": "Good First Issue",
        "problem": "Production operators running Waymesh nodes (universities, fleet co-ops) need metrics for monitoring node throughput, solver latencies, active journeys, OTP verification success/failure rates, and error frequencies.",
        "proposal": "Add a standard `/metrics` endpoint exporting Prometheus metrics using the `metrics` and `metrics-exporter-prometheus` crates.",
        "affected_files": [
            "crates/waymesh-server/Cargo.toml",
            "crates/waymesh-server/src/lib.rs"
        ],
        "steps": [
            "Add `metrics` and `metrics-exporter-prometheus` to `waymesh-server` dependencies.",
            "Instrument key counters and histograms: `waymesh_active_journeys`, `waymesh_active_bookings`, `waymesh_otp_verifications_total{status=\"success|failed\"}`, `waymesh_solver_duration_seconds`.",
            "Expose `GET /metrics` returning standard Prometheus plaintext format.",
            "Add automated test verifying `/metrics` responds with valid metric headers and values."
        ],
        "acceptance_criteria": [
            "GET `/metrics` returns 200 OK with Prometheus formatted counters and gauges.",
            "OTP failure lockout events increment `waymesh_otp_lockouts_total`.",
            "Zero impact on core request latency."
        ]
    },
    {
        "title": "[Deployment] Turnkey Docker Compose & Kubernetes Helm Chart with Caddy Auto-SSL",
        "labels": ["enhancement", "good first issue", "documentation"],
        "category": "DevOps & Infrastructure",
        "difficulty": "Good First Issue",
        "problem": "Community organizations, residential associations, and university clubs want to run their own Waymesh node in minutes without manual server configuration or SSL cert renewal.",
        "proposal": "Create a unified `docker-compose.prod.yml` and an official Helm chart packaging `waymesh-node`, PostgreSQL/PostGIS, and Caddy reverse proxy with automated Let's Encrypt SSL.",
        "affected_files": [
            "docker-compose.prod.yml",
            "Caddyfile",
            "deploy/helm/waymesh/Chart.yaml",
            "deploy/helm/waymesh/values.yaml"
        ],
        "steps": [
            "Create production `docker-compose.prod.yml` with health checks, automatic restart policies, and persistent volumes.",
            "Configure `Caddyfile` with automatic HTTPS reverse proxying to `waymesh-node:8080`.",
            "Create basic Helm chart with ConfigMap, Deployment, Service, and Ingress templates.",
            "Document 1-command startup in `docs/deployment/SELF_HOSTING.md`."
        ],
        "acceptance_criteria": [
            "`docker compose -f docker-compose.prod.yml up` starts healthy database, server, and SSL proxy.",
            "Server health check passes at `http://localhost:8080/health`.",
            "Documentation clearly tested on Ubuntu / Debian VPS."
        ]
    },
    {
        "title": "[Safety] Campus & Enterprise Domain Verification Adapter (.edu, .ac.in, corporate)",
        "labels": ["enhancement", "good first issue"],
        "category": "Trust, Safety & Identity",
        "difficulty": "Good First Issue",
        "problem": "For women-only carpooling and closed campus communities (e.g. CET, IISc, Technopark), users need a simple way to verify institutional affiliation without leaking personal credentials.",
        "proposal": "Add an email-based affiliation verification endpoint (`/api/v1/identities/:id/verify-email`) that sends a single-use verification code to institutional email domains (.edu, .ac.in, .org, or configured corporate domains) and marks the identity with verified trust claims.",
        "affected_files": [
            "crates/waymesh-core/src/lib.rs",
            "crates/waymesh-server/src/lib.rs",
            "crates/waymesh-server/src/db.rs"
        ],
        "steps": [
            "Add `verified_domains: Vec<String>` field to identity domain model in `waymesh-core`.",
            "Implement `POST /api/v1/identities/:id/request-email-verification` with rate limiting.",
            "Implement `POST /api/v1/identities/:id/confirm-email-verification` validating the 6-digit code.",
            "Allow filtering journeys and ride requests by required institutional domain."
        ],
        "acceptance_criteria": [
            "Users with verified `@cet.ac.in` domain can match with drivers requiring campus verification.",
            "Cross-domain or unverified users are filtered out when a journey specifies `required_affiliation`.",
            "Full unit test in `api_integration_tests.rs`."
        ]
    },
    {
        "title": "[Safety] Cryptographically Signed Ed25519 Peer Vouching & Community Trust Web",
        "labels": ["enhancement", "help wanted"],
        "category": "Trust, Safety & Identity",
        "difficulty": "Intermediate",
        "problem": "Centralized rating systems (like 5-star Uber reviews) are susceptible to platform manipulation, bias, and sudden account bans. Waymesh needs a decentralized Web-of-Trust where users cryptographically vouch for peers they have traveled with.",
        "proposal": "Implement Ed25519-signed vouchers in `waymesh-core`. After a successful Dual-OTP verified trip, Driver and Passenger can sign a `Vouch` attestment containing `{ vouch_target_id, trip_id, timestamp, score }`.",
        "affected_files": [
            "crates/waymesh-core/src/crypto.rs",
            "crates/waymesh-core/src/lib.rs",
            "crates/waymesh-server/src/lib.rs",
            "crates/waymesh-server/src/db.rs"
        ],
        "steps": [
            "Define `PeerVouch` struct with signature verification in `waymesh-core::crypto`.",
            "Add database table `peer_vouches` storing valid signatures linking passenger and driver.",
            "Compute an aggregated TrustScore based on verified vouch graph depth.",
            "Add API endpoint `POST /api/v1/identities/:id/vouch` requiring valid Ed25519 signature."
        ],
        "acceptance_criteria": [
            "Invalid signatures or forged vouchers are rejected with cryptographic error.",
            "Only parties with a completed `RideBooking` can submit mutual vouches for that trip.",
            "Unit tests verify tamper resistance."
        ]
    },
    {
        "title": "[Safety] Emergency Corridor Tripwire & Automated Route Deviation Alerting",
        "labels": ["enhancement", "help wanted"],
        "category": "Trust, Safety & Identity",
        "difficulty": "Intermediate",
        "problem": "If a vehicle veers significantly off its planned route corridor or stops unexpectedly for an abnormal duration during an active passenger ride, emergency contacts must be alerted immediately.",
        "proposal": "Create a background tripwire monitor in `waymesh-server`. If live driver telemetry deviates beyond `corridor_buffer_meters * 2` or is stationary in an unexpected location for > 20 minutes during `PickupConfirmed` status, emit an `emergency.tripwire_triggered` event.",
        "affected_files": [
            "crates/waymesh-server/src/lib.rs",
            "crates/waymesh-core/src/journey.rs"
        ],
        "steps": [
            "Add `EmergencyContact` field to user identities `{ name, phone, email }`.",
            "Implement periodic tripwire check function in server background worker.",
            "Compute geodesic distance between current telemetry point and route corridor centerline.",
            "Emit structured webhook / SSE event `emergency.tripwire_triggered` with GPS coordinates and journey details if boundary breached."
        ],
        "acceptance_criteria": [
            "Telemetry points inside the corridor trigger no alerts.",
            "Telemetry points exceeding buffer trigger emergency event within 5 seconds.",
            "Test scenario simulates deviation and verifies event dispatch."
        ]
    },
    {
        "title": "[Settlement] Private Peer-to-Peer Settlement Confirmation Webhook Adapter",
        "labels": ["enhancement", "help wanted"],
        "category": "Settlement & Private Payments",
        "difficulty": "Intermediate",
        "problem": "Waymesh mandates 100% peer-to-peer settlement directly between parties privately (via cash, direct peer fiat like UPI, or mutual arrangement) with zero platform custody or commission, and strictly bans speculative cryptocurrency. An automated webhook receiver is needed to verify private settlement confirmations between participants.",
        "proposal": "Implement an optional private settlement confirmation webhook adapter in `crates/waymesh-server` that listens for transfer/settlement confirmation webhooks and marks ride/parcel booking cost-shares as settled directly between parties privately.",
        "affected_files": [
            "crates/waymesh-server/src/lib.rs",
            "crates/waymesh-server/src/db.rs",
            "docs/settlement/SETTLEMENT_SPEC.md"
        ],
        "steps": [
            "Create `POST /api/v1/settlements/private/webhook` endpoint with HMAC-SHA256 signature verification.",
            "Match incoming payment reference against `booking_id` or `parcel_id`.",
            "Record settlement confirmation and transaction timestamp.",
            "Emit `settlement.completed` event on the node event bus."
        ],
        "acceptance_criteria": [
            "Valid webhook updates booking settlement status to `Settled` between parties privately.",
            "Unsigned or invalid HMAC webhooks are rejected with 401 Unauthorized.",
            "Zero crypto/token dependencies introduced."
        ]
    },
    {
        "title": "[Settlement] Dynamic UPI Intent Deep-Link & BharatQR Generation for In-Vehicle Fare Settlement",
        "labels": ["enhancement", "good first issue"],
        "category": "Settlement & Sound Money",
        "difficulty": "Good First Issue",
        "problem": "In India, UPI is the ubiquitous zero-fee direct bank rail. Passengers sitting in the car need an instant 1-tap payment link or QR code to transfer agreed fuel cost-shares directly to the driver's bank without third-party fees.",
        "proposal": "Add an endpoint `GET /api/v1/bookings/:id/upi-intent` that generates standard NPCI UPI intent URLs (`upi://pay?pa=...&pn=...&am=...&cu=INR&tn=Waymesh...`) and PNG QR codes.",
        "affected_files": [
            "crates/waymesh-server/src/lib.rs",
            "crates/waymesh-server/Cargo.toml"
        ],
        "steps": [
            "Add `qrcode` and `image` crates (or standard SVG generator) to `waymesh-server`.",
            "Build UPI intent URL string format conforming to NPCI UPI specifications.",
            "Return both raw intent URI and base64 SVG/PNG QR image in endpoint response.",
            "Add integration test verifying UPI string encoding with INR amount and booking reference."
        ],
        "acceptance_criteria": [
            "Generated UPI URL accurately formats driver VPA, passenger name, exact INR cost share, and booking ID.",
            "Endpoint returns in < 5ms.",
            "Direct peer-to-peer payment with zero platform commission."
        ]
    },
    {
        "title": "[Settlement] Mutual Digital Sign-off for Offline Cash Handover",
        "labels": ["enhancement", "good first issue"],
        "category": "Settlement & Sound Money",
        "difficulty": "Good First Issue",
        "problem": "Cash remains an essential offline settlement option for rural commuters and students. When physical cash notes are handed over, a mutual digital confirmation prevents fraudulent 'unpaid fare' dispute reports.",
        "proposal": "Add mutual sign-off support for cash settlements. Driver marks 'cash received'; passenger confirms with single-tap acknowledgement or 4-digit settlement receipt token.",
        "affected_files": [
            "crates/waymesh-core/src/booking.rs",
            "crates/waymesh-server/src/lib.rs"
        ],
        "steps": [
            "Add `CashSettlementStatus` enum (`PendingHandover`, `ReceivedByDriver`, `ConfirmedByPassenger`) to `RideBooking`.",
            "Implement `POST /api/v1/bookings/:id/settle-cash` allowing driver to register physical cash receipt.",
            "Passenger can acknowledge receipt or dispute discrepancy within 2 hours.",
            "Add unit tests for state transition in `booking.rs`."
        ],
        "acceptance_criteria": [
            "State machine prevents false disputes after mutual cash acknowledgement.",
            "Booking history logs timestamp and currency notes breakdown if specified.",
            "All tests pass."
        ]
    },
    {
        "title": "[Client] Lightweight Flutter / React Native Mobile Companion App with Offline OTP Vault",
        "labels": ["help wanted", "enhancement"],
        "category": "Developer Experience & Clients",
        "difficulty": "Help Wanted",
        "problem": "While conversational AI interfaces (MCP, ChatGPT Actions) make booking frictionless, drivers and passengers standing at roadside pickup points need an instant, low-bandwidth mobile app for QR codes, offline OTP display, and trip verification.",
        "proposal": "Create a minimalist mobile companion client in `apps/mobile` (Flutter or React Native / Expo) that stores user keys securely in the device keystore and caches active OTPs for offline viewing.",
        "affected_files": [
            "apps/mobile/README.md",
            "apps/mobile/pubspec.yaml"
        ],
        "steps": [
            "Scaffold minimalist mobile client in `apps/mobile`.",
            "Implement biometric / secure hardware key storage for user Ed25519 identity.",
            "Build 3 core screens: Corridor Search, Active Trip OTP Vault (large high-contrast digits for sunny outdoors), and Handover Scanner.",
            "Implement offline caching so active OTP tokens remain visible even with spotty cellular connection."
        ],
        "acceptance_criteria": [
            "App launches in < 1.5 seconds.",
            "Displays 6-digit pickup and delivery OTPs without active internet connection once trip is accepted.",
            "Clean open-source codebase with automated GitHub Actions build."
        ]
    },
    {
        "title": "[SDK] TypeScript / JavaScript Client SDK (@waymesh/sdk) with Full OpenAPI Typings",
        "labels": ["enhancement", "good first issue", "documentation"],
        "category": "Developer Experience & Clients",
        "difficulty": "Good First Issue",
        "problem": "Frontend web developers, campus portal creators, and bot builders need a zero-dependency, typed TypeScript library to interact with Waymesh nodes without writing manual HTTP requests.",
        "proposal": "Create `@waymesh/sdk` in `packages/sdk-ts` with automated type generation from `crates/waymesh-server/openapi.yaml`, supporting both Node.js and browser environments.",
        "affected_files": [
            "packages/sdk-ts/package.json",
            "packages/sdk-ts/src/index.ts",
            "crates/waymesh-server/openapi.yaml"
        ],
        "steps": [
            "Set up TypeScript package in `packages/sdk-ts` with `openapi-typescript` and `openapi-fetch`.",
            "Generate strongly typed client methods for all endpoints (journeys, bookings, parcels, MCP).",
            "Provide ergonomic helper classes for Ed25519 key generation and OTP verification.",
            "Write comprehensive README with 5-minute quickstart guide."
        ],
        "acceptance_criteria": [
            "`npm run build` generates clean ESM and CJS bundles with full `.d.ts` type declarations.",
            "Unit tests execute end-to-end trip creation against mock node in CI.",
            "Published ready for npm registry."
        ]
    },
    {
        "title": "[CLI] Interactive Terminal CLI (waymesh-cli) for Campus Logistics & Fleet Dispatch",
        "labels": ["enhancement", "good first issue"],
        "category": "Developer Experience & Clients",
        "difficulty": "Good First Issue",
        "problem": "System administrators, campus delivery coordinators, and terminal power users need a command-line interface to monitor corridor health, publish fleet journeys, and inspect parcel handovers directly from bash.",
        "proposal": "Create `crates/waymesh-cli` using `clap` and `ratatui` providing commands like `waymesh journey list`, `waymesh parcel request`, and `waymesh live-monitor`.",
        "affected_files": [
            "crates/waymesh-cli/Cargo.toml",
            "crates/waymesh-cli/src/main.rs",
            "Cargo.toml"
        ],
        "steps": [
            "Create new workspace crate `crates/waymesh-cli`.",
            "Implement subcommands with `clap(derive)`: `identity`, `journey`, `booking`, `parcel`, `status`.",
            "Add colorful terminal table output using `comfy-table`.",
            "Add `--json` output flag for scripting and piping to `jq`."
        ],
        "acceptance_criteria": [
            "`cargo run -p waymesh-cli -- journey list` connects to local node and formats active trips.",
            "`--json` produces clean parseable output.",
            "Installs cleanly via `cargo install --path crates/waymesh-cli`."
        ]
    },
    {
        "title": "[DX] Embedded Scalar / Swagger UI API Explorer at /docs",
        "labels": ["enhancement", "good first issue", "documentation"],
        "category": "Developer Experience & Clients",
        "difficulty": "Good First Issue",
        "problem": "Developers exploring a self-hosted Waymesh node currently have to inspect `openapi.yaml` manually. Serving an interactive, beautiful API documentation dashboard at `/docs` drastically improves developer onboarding.",
        "proposal": "Embed the modern Scalar API reference (or Swagger UI) directly into `waymesh-server`, serving it at `GET /docs` using the already embedded `openapi.yaml`.",
        "affected_files": [
            "crates/waymesh-server/src/lib.rs"
        ],
        "steps": [
            "Add route `GET /docs` in Axum router.",
            "Serve standalone Scalar HTML bundle that points to `/openapi.yaml` with modern dark/light mode UI.",
            "Add navigation link in README and server startup console banner.",
            "Add test verifying `GET /docs` returns 200 OK with HTML content."
        ],
        "acceptance_criteria": [
            "Navigating to `http://localhost:8080/docs` in browser renders interactive Scalar API explorer.",
            "All endpoints and schemas can be inspected and tested interactively.",
            "Zero runtime external asset dependencies if packaged with inline script."
        ]
    },
    {
        "title": "[Testing] High-Concurrency Corridor Traffic Simulation & Benchmark Suite",
        "labels": ["enhancement", "good first issue"],
        "category": "Testing & Quality Assurance",
        "difficulty": "Good First Issue",
        "problem": "Before deploying Waymesh to major university campuses (10,000+ daily commuters) or large metropolitan corridors, the community needs automated load testing scripts measuring match throughput and database connection pooling under heavy concurrent load.",
        "proposal": "Build a modular load testing suite in `benchmarks/load` using k6 or Python Locust that simulates 1,000 drivers publishing journeys and 5,000 riders/parcels searching corridors simultaneously.",
        "affected_files": [
            "benchmarks/load/corridor_load_test.js",
            "benchmarks/load/README.md"
        ],
        "steps": [
            "Write k6 load test script generating realistic geo-coordinates along realistic corridors.",
            "Measure p95 and p99 response times for `/api/v1/matches/search` and `/api/v1/journeys`.",
            "Output performance summary table with requests/sec and memory footprint.",
            "Add GitHub Actions benchmark trigger for performance regression tracking."
        ],
        "acceptance_criteria": [
            "Test simulates 1,000 concurrent virtual users sustained over 2 minutes.",
            "Maintains p95 response time under 50ms for in-memory matching.",
            "Clear instructions in `benchmarks/load/README.md`."
        ]
    },
    {
        "title": "[Protocol] Inter-Node Corridor Gossip & Peer Federated Discovery (Phase 2 RFC)",
        "labels": ["enhancement", "help wanted"],
        "category": "Decentralization & Protocol Scaling",
        "difficulty": "Advanced",
        "problem": "Waymesh is architected as an autonomous community protocol. Campus nodes (e.g. CET Trivandrum) and City nodes (e.g. Kochi Metro Co-op) need to peer with each other to exchange inter-city corridor trajectories without central authority.",
        "proposal": "Draft and prototype Phase 2 RFC for node-to-node gossip: federated corridor boundary exchange, signed trajectory propagation, and cross-node booking coordination.",
        "affected_files": [
            "docs/rfc/RFC_0002_FEDERATED_NODE_GOSSIP.md",
            "crates/waymesh-core/src/lib.rs"
        ],
        "steps": [
            "Write comprehensive RFC document defining node handshake, signed peer discovery, and privacy preservation.",
            "Specify inter-node message wire format over HTTPS or libp2p.",
            "Prototype cross-node search proxying: Node A queries Node B when corridor crosses node bounding box.",
            "Review threat model (Sybil attacks, corridor spamming)."
        ],
        "acceptance_criteria": [
            "Complete RFC specification document approved in `docs/rfc/`.",
            "Initial proof-of-concept wire test demonstrating federated query dispatch between 2 mock nodes.",
            "Adheres to zero-blockchain, pure cryptographic trust principles."
        ]
    },
    {
        "title": "[Security] Automated Cryptographic Audit & Memory Zeroization Hardening",
        "labels": ["enhancement", "help wanted"],
        "category": "Trust, Safety & Identity",
        "difficulty": "Intermediate",
        "problem": "To ensure enterprise-grade security for user cryptographic identities, secret keys in memory must be zeroized upon drop, and all hash comparisons must strictly use constant-time operations to prevent side-channel timing attacks.",
        "proposal": "Incorporate `zeroize` and `subtle` crates across `waymesh-core::crypto` to ensure Ed25519 private keys and OTP verification hashes are protected against memory dumping and timing analysis.",
        "affected_files": [
            "crates/waymesh-core/Cargo.toml",
            "crates/waymesh-core/src/crypto.rs",
            "crates/waymesh-core/src/booking.rs",
            "crates/waymesh-core/src/parcel.rs"
        ],
        "steps": [
            "Add `zeroize = { version = \"1.8\", features = [\"derive\"] }` and `subtle = \"2.6\"` to `waymesh-core`.",
            "Implement `Zeroize` and `ZeroizeOnDrop` for internal keypair memory structures.",
            "Replace standard string equality in OTP verification with `subtle::ConstantTimeEq`.",
            "Add security regression test verifying memory zeroization."
        ],
        "acceptance_criteria": [
            "All OTP comparisons execute in constant time regardless of character match position.",
            "Signing keys zeroize secret bytes on drop.",
            "Passes `cargo clippy` with zero warnings."
        ]
    }
]

def create_issue_on_github(token, issue):
    url = f"https://api.github.com/repos/{REPO}/issues"
    body_md = f"""## Problem Overview
{issue['problem']}

## Proposed Architecture & Solution
{issue['proposal']}

## Affected Crates & Files
{chr(10).join(f"- `{f}`" for f in issue['affected_files'])}

## Step-by-Step Implementation Guide
{chr(10).join(f"{i+1}. {step}" for i, step in enumerate(issue['steps']))}

## Definition of Done (Acceptance Criteria)
{chr(10).join(f"- [ ] {ac}" for ac in issue['acceptance_criteria'])}

---
* **Category:** {issue['category']}
* **Difficulty:** {issue['difficulty']}
* **Contribution Guide:** Please read [`CONTRIBUTING.md`](https://github.com/sahalhes/waymesh/blob/main/CONTRIBUTING.md) before submitting a PR.
"""
    data = {
        "title": issue["title"],
        "body": body_md,
        "labels": issue["labels"]
    }
    req = urllib.request.Request(
        url,
        data=json.dumps(data).encode("utf-8"),
        headers={
            "Authorization": f"Bearer {token}",
            "Accept": "application/vnd.github.v3+json",
            "Content-Type": "application/json",
            "User-Agent": "waymesh-issue-creator"
        }
    )
    try:
        with urllib.request.urlopen(req) as resp:
            res_data = json.loads(resp.read().decode("utf-8"))
            print(f"Created Issue #{res_data.get('number')}: {res_data.get('title')}")
            return res_data.get('number'), res_data.get('html_url')
    except urllib.error.HTTPError as e:
        err_msg = e.read().decode("utf-8")
        print(f"Failed to create issue '{issue['title']}': {e.code} - {err_msg}")
        return None, None

def generate_markdown_doc(created_issues):
    md = "# Community Roadmap & Contribution Issues (Startup Scaling & Open Source)\n\n"
    md += "Welcome to the Waymesh contributor community! Below are 20 curated, actionable issues designed to scale Waymesh from a solid protocol prototype into a high-scale community mobility startup and vibrant open-source ecosystem.\n\n"
    md += "Whether you are a Rust systems engineer, mobile developer, algorithms researcher, or DevOps specialist, there is a task scoped for you.\n\n"
    md += "To contribute:\n"
    md += "1. Find an issue below that matches your interest.\n"
    md += "2. Comment on the issue in the [GitHub Issue Tracker](https://github.com/sahalhes/waymesh/issues) to claim it.\n"
    md += "3. Follow our [CONTRIBUTING.md](../../CONTRIBUTING.md) to set up your environment, run tests, and open a PR.\n\n"
    md += "---\n\n"

    for i, issue in enumerate(ISSUES):
        gh_num = created_issues.get(i, {}).get("num")
        gh_url = created_issues.get(i, {}).get("url")
        link_str = f"[#{gh_num}]({gh_url})" if gh_num else f"Issue #{i+6}"
        labels_badge = " ".join([f"`{l}`" for l in issue["labels"]])
        
        md += f"### {link_str}: {issue['title']}\n"
        md += f"* **Category:** {issue['category']} | **Difficulty:** {issue['difficulty']}\n"
        md += f"* **Labels:** {labels_badge}\n"
        md += f"* **Problem:** {issue['problem']}\n"
        md += f"* **Files:** {', '.join([f'`{f}`' for f in issue['affected_files']])}\n\n"
        md += "**Implementation Steps:**\n"
        for s in issue["steps"]:
            md += f"- {s}\n"
        md += "\n**Acceptance Criteria:**\n"
        for ac in issue["acceptance_criteria"]:
            md += f"- [ ] {ac}\n"
        md += "\n---\n\n"

    return md

def update_bash_script():
    sh_content = """#!/usr/bin/env bash
# Automated GitHub Issue creation script for Waymesh
set -euo pipefail

REPO="sahalhes/waymesh"

TOKEN="${GITHUB_TOKEN:-}"
if [ -z "$TOKEN" ]; then
    if command -v git >/dev/null 2>&1; then
        TOKEN=$(printf "protocol=https\\nhost=github.com\\n" | git credential fill 2>/dev/null | grep "^password=" | cut -d= -f2 || true)
    fi
fi

if [ -z "$TOKEN" ]; then
    echo "ERROR: No GitHub token found. Please set GITHUB_TOKEN environment variable."
    exit 1
fi

echo "Running Python issue generator..."
python3 "$(dirname "$0")/../docs/contributing/sync_issues.py"
"""
    return sh_content

def main():
    token = get_github_token()
    if not token:
        print("ERROR: Could not get GitHub token from git credentials or environment!")
        sys.exit(1)

    print(f"Authenticated with GitHub. Creating {len(ISSUES)} issues on {REPO}...")
    created_map = {}
    for i, issue in enumerate(ISSUES):
        num, url = create_issue_on_github(token, issue)
        if num:
            created_map[i] = {"num": num, "url": url}
        time.sleep(0.5) # Gentle rate limiting

    print(f"Successfully posted {len(created_map)} issues to GitHub!")

    # Write COMMUNITY_ISSUES.md
    doc_path = "/home/sahalhes/projects/waymesh/docs/contributing/COMMUNITY_ISSUES.md"
    os.makedirs(os.path.dirname(doc_path), exist_ok=True)
    with open(doc_path, "w") as f:
        f.write(generate_markdown_doc(created_map))
    print(f"Updated documentation at {doc_path}")

if __name__ == "__main__":
    main()
