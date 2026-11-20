# Waymesh: Architecture for a P = NP Future

## 1. Problem Formulation

The core dispatch challenge in Waymesh is the **Multi-Commodity Dial-A-Ride Problem with Cargo Hitchhiking (MC-DARP-CH)**.

In computational complexity theory:
- DARP is known to be **NP-hard**.
- Finding optimal multi-hop vehicle routing with heterogeneous passenger and parcel capacity constraints cannot be solved in polynomial time unless **P = NP**.

## 2. The Architectural Dilemma

Most mobility systems make one of two mistakes:
1. **Hardcoding heuristic approximations directly into database queries:** (e.g. tightly coupling route matching to rigid SQL `ST_DWithin` filters), making it impossible to upgrade matching logic without changing database schemas and network APIs.
2. **Assuming a centralized supercomputer:** Building proprietary matching logic into a closed centralized server.

## 3. The Waymesh Solution: Decoupled Solver Abstraction

Waymesh establishes a strict boundary between the **Protocol & Data Layer** and the **Optimization Engine**:

```
┌────────────────────────────────────────────────────────┐
│                   Data & Node State                    │
│   Journeys, Parcels, Bookings in PostgreSQL/PostGIS    │
└───────────────────────────┬────────────────────────────┘
                            │
                            │ Candidate Pre-filtering (Spatial Bounding Box)
                            ▼
┌────────────────────────────────────────────────────────┐
│                 MatchProblem Struct                    │
│   In-memory slice: Candidate Journeys & Demand Items   │
└───────────────────────────┬────────────────────────────┘
                            │
                            │ MatchingSolver::solve(problem)
                            ▼
┌────────────────────────────────────────────────────────┐
│             MatchingSolver Trait (Swappable)           │
├───────────────────────────┬────────────────────────────┤
│ Today (P != NP):          │ Future (P = NP):           │
│ SpatialCorridorHeuristic  │ PolynomialOptimalSolver    │
│ Greedy O(N log M) vector  │ Exact global Pareto-optimal│
│ trajectory alignment      │ polynomial algorithm       │
└───────────────────────────┴────────────────────────────┘
                            │
                            ▼
┌────────────────────────────────────────────────────────┐
│                 SolverSolution Struct                  │
│   Assignments: { demand_id, journey_id, score, detour }│
└────────────────────────────────────────────────────────┘
```

### The Invariant Contract
When a polynomial-time breakthrough occurs:
1. A new struct implementing `MatchingSolver` is added to `waymesh-solver`.
2. The node operator sets `WAYMESH_SOLVER=polynomial_optimal` in configuration.
3. Zero database columns, API routes, or client MCP tools are modified.
