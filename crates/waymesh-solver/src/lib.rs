use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;
use waymesh_core::{Gender, GeoPoint};

#[derive(Error, Debug)]
pub enum SolverError {
    #[error("Internal solver calculation failure: {0}")]
    CalculationError(String),

    #[error("No feasible assignments found")]
    Infeasible,

    #[error("Timeout during solver computation")]
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorridorGeometry {
    pub polyline: Vec<GeoPoint>,
    pub buffer_meters: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JourneyCandidate {
    pub journey_id: Uuid,
    pub driver_id: Uuid,
    pub driver_gender: Gender,
    pub departure_time: DateTime<Utc>,
    pub available_seats: u32,
    pub available_cargo_weight_kg: f64,
    pub origin: GeoPoint,
    pub destination: GeoPoint,
    pub max_detour_minutes: u32,
    pub cost_per_seat: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DemandType {
    PassengerRide,
    ParcelDelivery,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemandItem {
    pub demand_id: Uuid,
    pub demand_type: DemandType,
    pub passenger_gender: Option<Gender>,
    pub pickup_location: GeoPoint,
    pub dropoff_location: GeoPoint,
    pub ready_time: DateTime<Utc>,
    pub deadline_time: DateTime<Utc>,
    pub seats_required: u32,
    pub cargo_weight_kg: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Objective {
    MinimizeTotalDetour,
    MaximizeCapacityUtilization,
    MinimizeCostPerPassenger,
    ParetoOptimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchProblem {
    pub journeys: Vec<JourneyCandidate>,
    pub demands: Vec<DemandItem>,
    pub optimization_objective: Objective,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchAssignment {
    pub demand_id: Uuid,
    pub journey_id: Uuid,
    pub estimated_detour_minutes: f64,
    pub match_score: f64, // 0.0 to 1.0 (higher is better)
    pub suggested_cost: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverSolution {
    pub assignments: Vec<MatchAssignment>,
    pub unassigned_demands: Vec<Uuid>,
    pub solver_identifier: String,
    pub computation_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlgorithmType {
    /// Fast greedy heuristic evaluating trajectory alignment and detour distance (O(N log M))
    SpatialHeuristic,
    /// Exact Branch-and-Bound / Mixed-Integer Linear Programming (Exponential worst-case)
    ExactMILP,
    /// Future polynomial-time optimal solver (P = NP theoretical adapter)
    PolyTimeOptimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverInfo {
    pub name: String,
    pub version: String,
    pub algorithm_type: AlgorithmType,
}

/// The core P = NP Future-Proofed Solver Trait.
/// Any matching algorithm (greedy, MILP, or future polynomial quantum solvers)
/// implements this single trait without modifying the database or protocol interfaces.
#[async_trait]
pub trait MatchingSolver: Send + Sync {
    fn info(&self) -> SolverInfo;
    async fn solve(&self, problem: MatchProblem) -> Result<SolverSolution, SolverError>;
}

/// Reference MVP implementation of SpatialCorridorHeuristic.
/// Scores candidate matches using spatial detour analysis and temporal alignment.
pub struct SpatialCorridorHeuristic {
    pub max_corridor_deviation_meters: f64,
}

impl Default for SpatialCorridorHeuristic {
    fn default() -> Self {
        Self {
            max_corridor_deviation_meters: 2500.0, // 2.5 km max detour corridor
        }
    }
}

impl SpatialCorridorHeuristic {
    pub fn new(max_deviation_meters: f64) -> Self {
        Self {
            max_corridor_deviation_meters: max_deviation_meters,
        }
    }

    /// Evaluates if a pickup and dropoff point fit within a journey trajectory.
    /// Returns Some((detour_minutes, score)) if within corridor detour threshold, None otherwise.
    pub fn score_corridor_fit(
        &self,
        origin: &GeoPoint,
        destination: &GeoPoint,
        max_detour_minutes: u32,
        pickup: &GeoPoint,
        dropoff: &GeoPoint,
    ) -> Option<(f64, f64)> {
        let base_journey_dist = origin.distance_meters(destination);
        let pickup_detour = origin.distance_meters(pickup);
        let ride_dist = pickup.distance_meters(dropoff);
        let return_dist = dropoff.distance_meters(destination);
        let total_routed_dist = pickup_detour + ride_dist + return_dist;

        let detour_meters = (total_routed_dist - base_journey_dist).max(0.0);
        if detour_meters > self.max_corridor_deviation_meters {
            return None;
        }

        let detour_minutes = detour_meters / 500.0;
        if detour_minutes > max_detour_minutes as f64 {
            return None;
        }

        let detour_penalty = (detour_meters / self.max_corridor_deviation_meters).min(1.0);
        let score = (1.0 - detour_penalty * 0.7).max(0.1);
        Some((detour_minutes, score))
    }
}

#[async_trait]
impl MatchingSolver for SpatialCorridorHeuristic {
    fn info(&self) -> SolverInfo {
        SolverInfo {
            name: "SpatialCorridorHeuristic".to_string(),
            version: "0.1.0".to_string(),
            algorithm_type: AlgorithmType::SpatialHeuristic,
        }
    }

    async fn solve(&self, problem: MatchProblem) -> Result<SolverSolution, SolverError> {
        let start = std::time::Instant::now();
        let mut assignments = Vec::new();
        let mut unassigned = Vec::new();

        // Track remaining capacity per journey across the batch solution run
        let mut remaining_seats: std::collections::HashMap<Uuid, u32> = problem
            .journeys
            .iter()
            .map(|j| (j.journey_id, j.available_seats))
            .collect();
        let mut remaining_cargo: std::collections::HashMap<Uuid, f64> = problem
            .journeys
            .iter()
            .map(|j| (j.journey_id, j.available_cargo_weight_kg))
            .collect();

        for demand in &problem.demands {
            let mut best_candidate: Option<(Uuid, f64, f64, u32)> = None;

            for journey in &problem.journeys {
                // Gender compatibility constraint check
                // Strict rule: In ride matching, do not match male and female
                if demand.demand_type == DemandType::PassengerRide {
                    if let Some(passenger_gender) = demand.passenger_gender {
                        if !passenger_gender.is_compatible_for_ride(&journey.driver_gender) {
                            continue;
                        }
                    }
                }

                // Capacity constraint check against tracked remaining capacity
                match demand.demand_type {
                    DemandType::PassengerRide => {
                        let avail = remaining_seats
                            .get(&journey.journey_id)
                            .copied()
                            .unwrap_or(0);
                        if avail < demand.seats_required {
                            continue;
                        }
                    }
                    DemandType::ParcelDelivery => {
                        let avail = remaining_cargo
                            .get(&journey.journey_id)
                            .copied()
                            .unwrap_or(0.0);
                        if avail < demand.cargo_weight_kg {
                            continue;
                        }
                    }
                }

                // Temporal window check (e.g. ready_time <= departure <= deadline)
                if journey.departure_time < demand.ready_time
                    || journey.departure_time > demand.deadline_time
                {
                    continue;
                }

                // Spatial detour calculation:
                // Direct distance of the journey
                let base_journey_dist = journey.origin.distance_meters(&journey.destination);

                // Distance if journey visits pickup then dropoff
                let pickup_detour = journey.origin.distance_meters(&demand.pickup_location);
                let ride_dist = demand
                    .pickup_location
                    .distance_meters(&demand.dropoff_location);
                let return_dist = demand
                    .dropoff_location
                    .distance_meters(&journey.destination);
                let total_routed_dist = pickup_detour + ride_dist + return_dist;

                let detour_meters = (total_routed_dist - base_journey_dist).max(0.0);

                if detour_meters > self.max_corridor_deviation_meters {
                    continue;
                }

                // Approximate detour time assuming 30 km/h urban speed = 500 m/min
                let detour_minutes = detour_meters / 500.0;
                if detour_minutes > journey.max_detour_minutes as f64 {
                    continue;
                }

                // Score between 0.0 and 1.0 (lower detour = higher score)
                let detour_penalty = (detour_meters / self.max_corridor_deviation_meters).min(1.0);
                let score = (1.0 - detour_penalty * 0.7).max(0.1);

                if let Some((_, _, best_score, _)) = best_candidate {
                    if score > best_score {
                        best_candidate = Some((
                            journey.journey_id,
                            detour_minutes,
                            score,
                            journey.cost_per_seat,
                        ));
                    }
                } else {
                    best_candidate = Some((
                        journey.journey_id,
                        detour_minutes,
                        score,
                        journey.cost_per_seat,
                    ));
                }
            }

            if let Some((journey_id, detour_minutes, score, cost)) = best_candidate {
                // Decrement assigned capacity so subsequent demands cannot oversubscribe
                match demand.demand_type {
                    DemandType::PassengerRide => {
                        if let Some(seats) = remaining_seats.get_mut(&journey_id) {
                            *seats = seats.saturating_sub(demand.seats_required);
                        }
                    }
                    DemandType::ParcelDelivery => {
                        if let Some(cargo) = remaining_cargo.get_mut(&journey_id) {
                            *cargo = (*cargo - demand.cargo_weight_kg).max(0.0);
                        }
                    }
                }

                assignments.push(MatchAssignment {
                    demand_id: demand.demand_id,
                    journey_id,
                    estimated_detour_minutes: detour_minutes,
                    match_score: score,
                    suggested_cost: cost,
                });
            } else {
                unassigned.push(demand.demand_id);
            }
        }

        let elapsed = start.elapsed().as_millis() as u64;

        Ok(SolverSolution {
            assignments,
            unassigned_demands: unassigned,
            solver_identifier: "SpatialCorridorHeuristic-v0.1".to_string(),
            computation_time_ms: elapsed,
        })
    }
}

/// Theoretical adapter demonstrating P = NP pluggability.
/// When practical polynomial-time algorithms are proven for NP-hard matching,
/// this struct implements MatchingSolver with zero external contract changes.
pub struct PolynomialOptimalSolverAdapter;

#[async_trait]
impl MatchingSolver for PolynomialOptimalSolverAdapter {
    fn info(&self) -> SolverInfo {
        SolverInfo {
            name: "PolynomialOptimalSolverAdapter (P=NP)".to_string(),
            version: "future-v1.0".to_string(),
            algorithm_type: AlgorithmType::PolyTimeOptimal,
        }
    }

    async fn solve(&self, problem: MatchProblem) -> Result<SolverSolution, SolverError> {
        // In current regime, delegates to SpatialCorridorHeuristic.
        // In future regime with polynomial algorithms, computes global optimum in O(n^k).
        let fallback = SpatialCorridorHeuristic::default();
        fallback.solve(problem).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_spatial_corridor_solver() {
        let solver = SpatialCorridorHeuristic::default();

        let origin = GeoPoint::new(12.9716, 77.5946); // Campus Gate
        let destination = GeoPoint::new(12.9780, 77.5696); // City Station
        let now = Utc::now();

        let journey = JourneyCandidate {
            journey_id: Uuid::new_v4(),
            driver_id: Uuid::new_v4(),
            driver_gender: Gender::Male,
            departure_time: now + chrono::Duration::minutes(30),
            available_seats: 3,
            available_cargo_weight_kg: 15.0,
            origin,
            destination,
            max_detour_minutes: 15,
            cost_per_seat: 8000,
        };

        // Demand within 500m of corridor (Male passenger matches Male driver)
        let pickup = GeoPoint::new(12.9725, 77.5900);
        let dropoff = GeoPoint::new(12.9770, 77.5710);

        let demand = DemandItem {
            demand_id: Uuid::new_v4(),
            demand_type: DemandType::PassengerRide,
            passenger_gender: Some(Gender::Male),
            pickup_location: pickup,
            dropoff_location: dropoff,
            ready_time: now,
            deadline_time: now + chrono::Duration::hours(2),
            seats_required: 1,
            cargo_weight_kg: 0.0,
        };

        let problem = MatchProblem {
            journeys: vec![journey],
            demands: vec![demand],
            optimization_objective: Objective::MinimizeTotalDetour,
        };

        let solution = solver.solve(problem).await.expect("Solver should succeed");
        assert_eq!(solution.assignments.len(), 1);
        assert_eq!(solution.unassigned_demands.len(), 0);
        assert!(solution.assignments[0].match_score > 0.5);
    }

    #[tokio::test]
    async fn test_gender_separation_male_female_does_not_match() {
        let solver = SpatialCorridorHeuristic::default();

        let origin = GeoPoint::new(12.9716, 77.5946);
        let destination = GeoPoint::new(12.9780, 77.5696);
        let now = Utc::now();

        // Male driver journey
        let male_journey = JourneyCandidate {
            journey_id: Uuid::new_v4(),
            driver_id: Uuid::new_v4(),
            driver_gender: Gender::Male,
            departure_time: now + chrono::Duration::minutes(30),
            available_seats: 3,
            available_cargo_weight_kg: 15.0,
            origin: origin.clone(),
            destination: destination.clone(),
            max_detour_minutes: 15,
            cost_per_seat: 8000,
        };

        // Female passenger ride demand along the exact same corridor
        let female_demand = DemandItem {
            demand_id: Uuid::new_v4(),
            demand_type: DemandType::PassengerRide,
            passenger_gender: Some(Gender::Female),
            pickup_location: GeoPoint::new(12.9725, 77.5900),
            dropoff_location: GeoPoint::new(12.9770, 77.5710),
            ready_time: now,
            deadline_time: now + chrono::Duration::hours(2),
            seats_required: 1,
            cargo_weight_kg: 0.0,
        };

        let problem = MatchProblem {
            journeys: vec![male_journey],
            demands: vec![female_demand.clone()],
            optimization_objective: Objective::MinimizeTotalDetour,
        };

        let solution = solver.solve(problem).await.expect("Solver should execute");
        // Strict requirement: male and female MUST NOT match
        assert_eq!(
            solution.assignments.len(),
            0,
            "Male driver and female passenger must not match"
        );
        assert_eq!(solution.unassigned_demands.len(), 1);
        assert_eq!(solution.unassigned_demands[0], female_demand.demand_id);
    }

    #[tokio::test]
    async fn test_same_gender_ride_matches() {
        let solver = SpatialCorridorHeuristic::default();

        let origin = GeoPoint::new(12.9716, 77.5946);
        let destination = GeoPoint::new(12.9780, 77.5696);
        let now = Utc::now();

        // Female driver journey
        let female_journey = JourneyCandidate {
            journey_id: Uuid::new_v4(),
            driver_id: Uuid::new_v4(),
            driver_gender: Gender::Female,
            departure_time: now + chrono::Duration::minutes(30),
            available_seats: 3,
            available_cargo_weight_kg: 15.0,
            origin: origin.clone(),
            destination: destination.clone(),
            max_detour_minutes: 15,
            cost_per_seat: 8000,
        };

        // Female passenger ride demand
        let female_demand = DemandItem {
            demand_id: Uuid::new_v4(),
            demand_type: DemandType::PassengerRide,
            passenger_gender: Some(Gender::Female),
            pickup_location: GeoPoint::new(12.9725, 77.5900),
            dropoff_location: GeoPoint::new(12.9770, 77.5710),
            ready_time: now,
            deadline_time: now + chrono::Duration::hours(2),
            seats_required: 1,
            cargo_weight_kg: 0.0,
        };

        let problem = MatchProblem {
            journeys: vec![female_journey.clone()],
            demands: vec![female_demand],
            optimization_objective: Objective::MinimizeTotalDetour,
        };

        let solution = solver.solve(problem).await.expect("Solver should execute");
        assert_eq!(
            solution.assignments.len(),
            1,
            "Female driver and female passenger should match"
        );
        assert_eq!(
            solution.assignments[0].journey_id,
            female_journey.journey_id
        );
    }

    #[tokio::test]
    async fn test_parcel_delivery_is_gender_agnostic() {
        let solver = SpatialCorridorHeuristic::default();

        let origin = GeoPoint::new(12.9716, 77.5946);
        let destination = GeoPoint::new(12.9780, 77.5696);
        let now = Utc::now();

        // Male driver journey
        let male_journey = JourneyCandidate {
            journey_id: Uuid::new_v4(),
            driver_id: Uuid::new_v4(),
            driver_gender: Gender::Male,
            departure_time: now + chrono::Duration::minutes(30),
            available_seats: 3,
            available_cargo_weight_kg: 15.0,
            origin: origin.clone(),
            destination: destination.clone(),
            max_detour_minutes: 15,
            cost_per_seat: 8000,
        };

        // Parcel delivery request has no passenger physical presence in cabin
        let parcel_demand = DemandItem {
            demand_id: Uuid::new_v4(),
            demand_type: DemandType::ParcelDelivery,
            passenger_gender: None,
            pickup_location: GeoPoint::new(12.9725, 77.5900),
            dropoff_location: GeoPoint::new(12.9770, 77.5710),
            ready_time: now,
            deadline_time: now + chrono::Duration::hours(2),
            seats_required: 0,
            cargo_weight_kg: 3.0,
        };

        let problem = MatchProblem {
            journeys: vec![male_journey.clone()],
            demands: vec![parcel_demand],
            optimization_objective: Objective::MinimizeTotalDetour,
        };

        let solution = solver.solve(problem).await.expect("Solver should execute");
        assert_eq!(
            solution.assignments.len(),
            1,
            "Parcels are transported regardless of carrier gender"
        );
        assert_eq!(solution.assignments[0].journey_id, male_journey.journey_id);
    }

    #[tokio::test]
    async fn test_batch_solver_capacity_limit_enforcement() {
        let solver = SpatialCorridorHeuristic::default();
        let origin = GeoPoint::new(12.9716, 77.5946);
        let destination = GeoPoint::new(12.9780, 77.5696);
        let now = Utc::now();

        // Journey has exactly 2 seats available
        let journey = JourneyCandidate {
            journey_id: Uuid::new_v4(),
            driver_id: Uuid::new_v4(),
            driver_gender: Gender::Male,
            departure_time: now + chrono::Duration::minutes(30),
            available_seats: 2,
            available_cargo_weight_kg: 5.0,
            origin: origin.clone(),
            destination: destination.clone(),
            max_detour_minutes: 15,
            cost_per_seat: 8000,
        };

        // Demand 1 requires 2 seats
        let demand1 = DemandItem {
            demand_id: Uuid::new_v4(),
            demand_type: DemandType::PassengerRide,
            passenger_gender: Some(Gender::Male),
            pickup_location: GeoPoint::new(12.9725, 77.5900),
            dropoff_location: GeoPoint::new(12.9770, 77.5710),
            ready_time: now,
            deadline_time: now + chrono::Duration::hours(2),
            seats_required: 2,
            cargo_weight_kg: 0.0,
        };

        // Demand 2 also requires 2 seats along the same corridor
        let demand2 = DemandItem {
            demand_id: Uuid::new_v4(),
            demand_type: DemandType::PassengerRide,
            passenger_gender: Some(Gender::Male),
            pickup_location: GeoPoint::new(12.9725, 77.5900),
            dropoff_location: GeoPoint::new(12.9770, 77.5710),
            ready_time: now,
            deadline_time: now + chrono::Duration::hours(2),
            seats_required: 2,
            cargo_weight_kg: 0.0,
        };

        let problem = MatchProblem {
            journeys: vec![journey],
            demands: vec![demand1.clone(), demand2.clone()],
            optimization_objective: Objective::MinimizeTotalDetour,
        };

        let solution = solver.solve(problem).await.expect("Solver should execute");
        // Only one demand should be assigned; the other must remain unassigned due to capacity limit
        assert_eq!(
            solution.assignments.len(),
            1,
            "Only 1 demand should fit in the 2-seat capacity"
        );
        assert_eq!(
            solution.unassigned_demands.len(),
            1,
            "The second demand must be unassigned"
        );
    }
}
