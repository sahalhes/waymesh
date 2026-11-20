# Waymesh: Open Protocol for Decentralized Mobility and Logistics

[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Build & Test](https://img.shields.io/badge/Build-Passing-brightgreen.svg)]()

> **Every journey carries possibility.**

Waymesh is an open protocol and decentralized network for coordinating:
- **Peer-to-peer ride sharing & carpooling**
- **Community parcel delivery & cargo hitchhiking**
- **Distributed transportation capacity**

Instead of forcing users onto a monolithic app store download, Waymesh operates as a **headless, agent-native protocol**. Independent community nodes expose conversational tool connectors to **ChatGPT, Claude (via Model Context Protocol - MCP), and OpenWebUI**, allowing commuters to publish journeys and request rides or deliveries directly in natural language.

---

## Architecture Overview

Waymesh is engineered in high-performance Rust and designed with a pluggable **Solver Abstraction Layer (P = NP Future-Proofed)**:

```
                  ┌────────────────────────────────────────────────────────┐
                  │                 CONVERSATIONAL CLIENTS                 │
                  │  Claude Desktop / Web    ChatGPT Actions    OpenWebUI  │
                  └───────────┬─────────────────────┬───────────────┬──────┘
                              │ (MCP / JSON-RPC)    │ (OpenAPI REST)│ (REST/SSE)
                              ▼                     ▼               ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                             WAYMESH NODE                                 │
│                                                                          │
│  ┌────────────────────────┐  ┌────────────────────────────────────────┐  │
│  │   waymesh-mcp          │  │   waymesh-server                       │  │
│  │   (Model Context Proto)│  │   (Axum REST, SSE, WebSockets)         │  │
│  └───────────┬────────────┘  └────────────────────┬───────────────────┘  │
│              │                                    │                      │
│              ▼                                    ▼                      │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │   waymesh-core                                                     │  │
│  │   - Identity & DID / Ed25519 Signatures                            │  │
│  │   - Journey Lifecycle & Corridor Management                        │  │
│  │   - Parcel Logistics & Custody State Machine                       │  │
│  │   - Verifiable Handover & Dual OTP Generation                      │  │
│  └─────────────────────────────────┬──────────────────────────────────┘  │
│                                    │                                     │
│                                    ▼                                     │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │   waymesh-solver (P = NP Ready Abstraction)                        │  │
│  │   - Trait MatchingSolver: MatchProblem -> Solution                 │  │
│  │   - SpatialCorridorHeuristic (In-Memory Trajectory Scoring)        │  │
│  │   - PolynomialOptimalSolverAdapter (Theoretical Future Spec)       │  │
│  └─────────────────────────────────┬──────────────────────────────────┘  │
│                                    │                                     │
└────────────────────────────────────┼─────────────────────────────────────┘
                                     │
                 ┌───────────────────┴───────────────────┐
                 ▼                                       ▼
  ┌───────────────────────────────┐     ┌─────────────────────────────────┐
  │   PostgreSQL + PostGIS        │     │   Routing Engine (Optional)     │
  │   - Spatial Candidate Filter  │     │   - OSRM / Valhalla / Mapbox    │
  │     (ST_DWithin, ST_Buffer)   │     │   - Geodesic fallback built-in  │
  │   - Journeys, Parcels, Bookings│    └─────────────────────────────────┘
  └───────────────────────────────┘
```

---

## Monorepo Layout

```
waymesh/
├── Cargo.toml                    # Root workspace definition
├── README.md                     # Vision, architecture, and quickstart guide
├── LICENSE                       # Apache-2.0
├── docker-compose.yml            # Turnkey local development stack
├── scripts/
│   └── demo_corridor.py          # End-to-end corridor demonstration script
├── deployments/
│   └── docker/
│       ├── Dockerfile.node       # Multi-stage optimized Rust build
│       └── postgis-init/
│           └── 01_init.sql       # PostgreSQL + PostGIS schema & corridor seeds
├── crates/
│   ├── waymesh-core/             # Identities, Journeys, Bookings, Parcels, OTP crypto
│   ├── waymesh-solver/           # P=NP MatchingSolver trait & SpatialCorridorHeuristic
│   ├── waymesh-server/           # Axum HTTP REST, SSE event stream, and state store
│   └── waymesh-mcp/              # Anthropic Model Context Protocol server (stdio & JSON-RPC)
├── connectors/
│   ├── chatgpt/
│   │   ├── waymesh-plugin/       # Official OpenAI Agent Plugin package & Custom GPT Actions
│   │   └── openapi.yaml          # OpenAPI 3.1 specification for Custom GPT Actions
│   ├── claude/
│   │   ├── waymesh-plugin/       # Official Claude Code & Anthropic Directory Plugin (.claude-plugin, .mcp.json, skills)
│   │   └── README.md             # Claude installation & marketplace guide
│   └── openwebui/
│       └── tool_waymesh.py       # Python function calling tool for OpenWebUI
└── docs/
    ├── protocol/
    │   └── RFC-0001-CORE-PROTOCOL.md
    ├── architecture/
    │   └── SOLVER-P-VS-NP.md
    └── designs/
        └── waymesh-protocol.md
```

---

## P = NP Optimization Abstraction

In standard complexity theory, multi-commodity dial-a-ride with cargo hitchhiking is strongly **NP-hard**.

Waymesh abstracts optimization behind the `MatchingSolver` trait:

```rust
#[async_trait]
pub trait MatchingSolver: Send + Sync {
    fn info(&self) -> SolverInfo;
    async fn solve(&self, problem: MatchProblem) -> Result<SolverSolution, SolverError>;
}
```

- **Today (P != NP):** Uses `SpatialCorridorHeuristic` (vector trajectory alignment, detour bounding, and Haversine distance ranking in `< 15ms`).
- **Hypothetical Future (P = NP):** When constructive polynomial-time algorithms are available, `PolynomialOptimalSolverAdapter` computes global Pareto-optimal dispatches across thousands of nodes in $O(n^k)$ with **zero changes** to database columns, API routes, or client connectors.

---

## Connecting Conversational AI Clients

### 1. Claude (Claude Code, Claude Desktop, & Anthropic Directory)

#### A. Claude Code Plugin (One-Command Install via Marketplace)
```bash
# Add Waymesh official marketplace
claude plugin marketplace add sahalhes/waymesh

# Install Waymesh Mobility plugin
claude plugin install waymesh-mobility
```
Or run directly for a single session:
```bash
claude --plugin-dir connectors/claude/waymesh-plugin
```

#### B. Claude Desktop (Remote HTTP MCP)
Add to your `claude_desktop_config.json`:
```json
{
  "mcpServers": {
    "waymesh": {
      "type": "http",
      "url": "https://waymesh.icvalley.tech/mcp"
    }
  }
}
```

Now prompt Claude naturally:
> *"I need safe carpooling from CET to Kochuveli Railway Station today at 5:00 PM. Female passenger."*
> *"Publish a commuter journey from Campus North Gate to City Central Railway Station with 3 seats at 5:00 PM."*

### 2. ChatGPT (Agent Plugin & Custom Actions)
- **Official Agent Plugin**: Upload `connectors/chatgpt/waymesh-plugin.zip` (or root `waymesh-plugin.zip`) containing `.well-known/ai-plugin.json`, `mcp.json`, and OpenAPI definitions.
- **Custom GPT Actions**: Import `connectors/chatgpt/openapi.yaml` directly into your Custom GPT Actions dashboard.

### 3. OpenWebUI
Drop `connectors/openwebui/tool_waymesh.py` into OpenWebUI's custom tool catalog to enable native tool calling across open-source models (Llama, Mistral, Qwen, DeepSeek).

---

## Bidirectional Ride Protocol: Check or Offer ("Check Ride is There or Offer if Not Present")

Waymesh supports symmetrical, bidirectional mobility coordination:
- **Check if a Ride is Present:** Commuters search active driver corridors (`waymesh_search_matches`). If an empty seat is found, they book it with zero technical friction (`waymesh_create_booking`).
- **Offer or Request if Not Present:**
  - **Passenger Commuter Demand:** If no driver has published a matching route, the passenger seamlessly publishes a **Ride Request** (`waymesh_publish_ride_request`).
  - **Driver Fulfillments & Corridor Alerts:** Drivers heading along that corridor can browse open ride requests (`waymesh_search_ride_requests`), or publish their journey (`waymesh_publish_journey`), which automatically notifies them of waiting passengers.
  - **One-Click Acceptance & Handover OTP:** Drivers fulfill passenger requests with `waymesh_accept_ride_request`, immediately generating the cryptographic 6-digit pickup OTP.
  - **Automatic Landmark Coordinate Resolution:** Built-in resolution for college campuses, transit hubs, and tech corridors (CET Trivandrum, TVM Central, Technopark, Kochuveli, Bengaluru) means users never need to manually look up lat/lng coordinates.

---

## Safe Carpooling Protocol: Strict Gender Matching Policy

Safe carpooling is the foundational security guarantee of the Waymesh protocol. To ensure participant safety, mutual comfort, and trust across community transport:
- **Male and female riders are NEVER matched together in passenger rides.**
- **Male passengers** match strictly with **male drivers** and co-riders.
- **Female passengers** match strictly with **female drivers** and co-riders.
- **Multi-layered Enforcement:**
  1. **Matching Solver Level:** Incompatible driver/passenger pairings are pruned during candidate corridor evaluation.
  2. **API Gate Level:** The `/api/v1/bookings` endpoint strictly verifies gender compatibility before reserving seats and rejects cross-gender attempts with HTTP `403 Forbidden`.
- **Parcel Logistics Exemption:** Cargo crowdshipping takes place in the vehicle trunk/cargo area without physical passenger cabin co-presence, meaning parcel delivery remains gender-agnostic.

---

## Verifiable Handover Protocol (Two-Stage OTP)

To eliminate disputes without fiat escrow lockups, parcel logistics utilizes dual cryptographically hashed one-time passwords:

```
[PUBLISHED] 
     │ (Carrier Accepts)
     ▼
[PICKUP_PENDING]  ─── Generates Pickup OTP (given to Sender)
     │                 & Delivery OTP (given to Recipient)
     │ (Physical Meetup: Sender discloses Pickup OTP)
     ▼
[IN_TRANSIT]      ─── Carrier verifies Pickup OTP. Legal custody transfers.
     │
     │ (Destination Hub: Recipient inspects & discloses Delivery OTP)
     ▼
[DELIVERED]       ─── Carrier verifies Delivery OTP. Custody released.
```

---

## Settlement & Private Peer-to-Peer Payments (Strict No-Crypto Policy)

Waymesh is intentionally built around real, sovereign economic value and strictly rejects speculative financial tokens:
- **Strictly No Cryptocurrencies / No Web3:** Zero crypto coins, zero speculative tokens, zero blockchain gas fees.
- **Private Peer-to-Peer Settlements:** All cost shares and compensations are settled directly between parties privately. Waymesh never takes custody of funds or charges platform commissions.
- **Direct Sovereign Cash Handover:** Direct hand-to-hand cash payment at pickup or delivery with zero fees and total privacy.
- **Direct Peer Digital Fiat (UPI / Instant Bank Transfer):** Direct account-to-account payments using national rails (e.g. UPI QR/handles) without platform custody or commissions.

---

## Quickstart

### Run with Docker Compose
```bash
docker compose up -d
```
Check health:
```bash
curl http://localhost:8080/health
```

### Run the Interactive Corridor Demo
Simulates the entire Campus North Gate ↔ City Central Railway Station workflow with ride booking, dual OTP verification, parcel crowdshipping, and sound-money settlement:
```bash
python3 scripts/demo_corridor.py
```

### Build & Test Natively
```bash
# Run all unit and integration tests across the workspace
cargo test --workspace
```

---

## Governance & RFC Process

- **License:** Apache License 2.0.
- **Protocol RFCs:** Changes to wire schemas and state transitions follow the RFC process in `docs/protocol/`.
- **Node Conformance:** Independent node implementations in Go, Python, or TypeScript can verify compliance against the test suite.
