use crate::error::{CoreError, Result};
use crate::identity::Gender;
use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum JourneyStatus {
    Draft,
    Active,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GeoPoint {
    pub latitude: f64,
    pub longitude: f64,
}

impl GeoPoint {
    pub fn new(latitude: f64, longitude: f64) -> Self {
        Self {
            latitude,
            longitude,
        }
    }

    /// Check if coordinates are finite and within valid Earth geographic bounds
    pub fn is_valid(&self) -> bool {
        self.latitude.is_finite()
            && self.longitude.is_finite()
            && (-90.0..=90.0).contains(&self.latitude)
            && (-180.0..=180.0).contains(&self.longitude)
    }

    /// Calculate distance in meters using Haversine formula
    pub fn distance_meters(&self, other: &GeoPoint) -> f64 {
        const EARTH_RADIUS_METERS: f64 = 6_371_000.0;
        let d_lat = (other.latitude - self.latitude).to_radians();
        let d_lon = (other.longitude - self.longitude).to_radians();
        let lat1 = self.latitude.to_radians();
        let lat2 = other.latitude.to_radians();

        let a = (d_lat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (d_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().asin();

        EARTH_RADIUS_METERS * c
    }
}

/// Normalize location string by lowercasing, replacing punctuation with spaces, and trimming
fn normalize_location_str(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Automatically resolve well-known landmarks to approximate latitude/longitude
pub fn resolve_known_landmark(name: &str) -> Option<GeoPoint> {
    let lower = normalize_location_str(name);
    if lower.is_empty() {
        return None;
    }

    // 1. College of Engineering Trivandrum (CET) & Sreekaryam
    if lower.contains("cet")
        || lower.contains("college of engineering")
        || lower.contains("sreekaryam")
        || lower.contains("sreekariyam")
        || lower.contains("engineering college trivandrum")
    {
        Some(GeoPoint::new(8.5458, 76.9063))
    }
    // 2. Thiruvananthapuram Central Railway Station & Thampanoor (KSRTC Central)
    else if lower.contains("tvm station")
        || lower.contains("trivandrum central")
        || lower.contains("tvm central")
        || lower.contains("tvm railway")
        || lower.contains("trivandrum railway")
        || lower.contains("thampanoor")
        || lower.contains("thampanoor station")
        || lower.contains("thampanoor bus")
        || lower.contains("tvc")
        || lower.contains("thiruvananthapuram central")
        || lower.contains("trivandrum railway station")
        || lower.contains("railway station tvm")
    {
        Some(GeoPoint::new(8.4875, 76.9525))
    }
    // Chavadimukku & Pangappara (Trivandrum)
    else if lower.contains("chavadimukku")
        || lower.contains("chavadimuk")
        || lower.contains("chavadi")
    {
        Some(GeoPoint::new(8.5372, 76.9048))
    } else if lower.contains("pangappara") {
        Some(GeoPoint::new(8.5480, 76.8970))
    }
    // 3. Kochuveli Railway Station (KCVL)
    else if lower.contains("kochuveli") || lower.contains("kcvl") {
        Some(GeoPoint::new(8.5085, 76.9038))
    }
    // 4. Technopark Trivandrum (Phases 1, 2, 3, 4, Technocity)
    else if lower.contains("technopark")
        || lower.contains("thejaswini")
        || lower.contains("tejaswini")
        || lower.contains("bhavani building")
        || lower.contains("technocity")
    {
        Some(GeoPoint::new(8.5580, 76.8770))
    }
    // 5. Kazhakkoottam / Kazhakuttam Junction
    else if lower.contains("kazhakkoottam")
        || lower.contains("kazhakuttam")
        || lower.contains("kazhakoottam")
        || lower.contains("al saj")
    {
        Some(GeoPoint::new(8.5714, 76.8687))
    }
    // 6. Lulu Mall Thiruvananthapuram & Akkulam
    else if lower.contains("lulu mall")
        || lower.contains("lulu tvm")
        || lower.contains("lulu trivandrum")
        || lower.contains("akkulam")
    {
        Some(GeoPoint::new(8.5303, 76.9011))
    }
    // 7. Kempegowda Airport Bangalore / BLR
    else if lower.contains("kempegowda")
        || lower.contains("blr airport")
        || (lower.contains("bangalore") && lower.contains("airport"))
        || (lower.contains("bengaluru") && lower.contains("airport"))
    {
        Some(GeoPoint::new(13.1986, 77.7066))
    }
    // 8. Cochin International Airport (COK / Nedumbassery)
    else if lower.contains("cochin airport")
        || lower.contains("nedumbassery")
        || lower.contains("cok")
    {
        Some(GeoPoint::new(10.1518, 76.3930))
    }
    // 9. Trivandrum International Airport (TRV)
    else if lower.contains("airport")
        || lower.contains("trv")
        || lower.contains("shankhumukham")
        || lower.contains("chakkai")
    {
        Some(GeoPoint::new(8.4821, 76.9200))
    }
    // 10. Medical College Hospital (MCH) Trivandrum & Ulloor
    else if lower.contains("medical college")
        || lower.contains("mch trivandrum")
        || lower.contains("mch tvm")
        || lower.contains("ulloor")
        || lower.contains("kumarapuram")
    {
        Some(GeoPoint::new(8.5238, 76.9278))
    }
    // 11. East Fort (Kizhakkekotta) & Padmanabhaswamy Temple
    else if lower.contains("east fort")
        || lower.contains("kizhakkekotta")
        || lower.contains("padmanabhaswamy")
        || lower.contains("attukal")
        || lower.contains("chalai")
    {
        Some(GeoPoint::new(8.4840, 76.9450))
    }
    // 12. Palayam, Museum, Vellayambalam, Secretariat, Statue
    else if lower.contains("palayam")
        || lower.contains("secretariat")
        || lower.contains("statue tvm")
        || lower.contains("statue trivandrum")
        || lower.contains("museum tvm")
        || lower.contains("vellayambalam")
        || lower.contains("kowdiar")
    {
        Some(GeoPoint::new(8.5048, 76.9526))
    }
    // 13. Pettah Railway Station
    else if lower.contains("pettah") || lower.contains("pattoor") {
        Some(GeoPoint::new(8.4975, 76.9312))
    }
    // 14. Kariavattom (Kerala University Campus / Greenfield Sports Hub)
    else if lower.contains("kariavattom")
        || lower.contains("karyavattom")
        || lower.contains("greenfield")
        || lower.contains("sports hub")
    {
        Some(GeoPoint::new(8.5684, 76.8837))
    }
    // 15. Kovalam Beach / Vizhinjam Port
    else if lower.contains("kovalam") || lower.contains("vizhinjam") {
        Some(GeoPoint::new(8.4004, 76.9787))
    }
    // 16. Varkala Cliff / Station
    else if lower.contains("varkala") {
        Some(GeoPoint::new(8.7379, 76.7163))
    }
    // 17. General Trivandrum / Thiruvananthapuram / TVM Hub
    else if lower.contains("thiruvananthapuram")
        || lower.contains("trivandrum")
        || lower == "tvm"
        || lower.starts_with("tvm ")
        || lower.ends_with(" tvm")
        || lower.contains(" tvm ")
        || lower == "tvc"
    {
        Some(GeoPoint::new(8.4875, 76.9525))
    }
    // 18. Kochi / Ernakulam Locations
    else if lower.contains("ernakulam south")
        || lower.contains("ers")
        || lower.contains("south railway station kochi")
    {
        Some(GeoPoint::new(9.9678, 76.2890))
    } else if lower.contains("infopark") || lower.contains("kakkanad") {
        Some(GeoPoint::new(10.0104, 76.3630))
    } else if lower.contains("edappally") || lower.contains("lulu kochi") {
        Some(GeoPoint::new(10.0270, 76.3080))
    } else if lower.contains("ernakulam north")
        || lower.contains("ernakulam")
        || lower.contains("kochi")
        || lower.contains("cochin")
        || lower == "ern"
        || lower.starts_with("ern ")
        || lower.ends_with(" ern")
        || lower.contains(" ern ")
        || lower.contains("town station ernakulam")
    {
        Some(GeoPoint::new(9.9938, 76.2858))
    }
    // 18. Bengaluru Locations
    else if lower.contains("campus north gate")
        || (lower.contains("bengaluru") && lower.contains("gate"))
    {
        Some(GeoPoint::new(12.9716, 77.5946))
    } else if lower.contains("city central railway station")
        || lower.contains("majestic")
        || lower.contains("ksr bengaluru")
        || (lower.contains("bengaluru") && lower.contains("station"))
    {
        Some(GeoPoint::new(12.9780, 77.5700))
    } else if lower.contains("electronic city") || lower.contains("ecity") {
        Some(GeoPoint::new(12.8452, 77.6602))
    } else if lower.contains("whitefield") {
        Some(GeoPoint::new(12.9698, 77.7499))
    } else if lower.contains("koramangala") {
        Some(GeoPoint::new(12.9352, 77.6245))
    } else if lower.contains("indiranagar") {
        Some(GeoPoint::new(12.9784, 77.6408))
    } else if lower.contains("hsr layout") || lower.contains("hsr") {
        Some(GeoPoint::new(12.9121, 77.6446))
    } else if lower.contains("silk board") {
        Some(GeoPoint::new(12.9177, 77.6238))
    } else {
        None
    }
}

/// Parse human or semi-structured datetime strings into UTC DateTime
pub fn parse_flexible_datetime(input: &str) -> Option<DateTime<Utc>> {
    let s = input.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("now") || s.eq_ignore_ascii_case("immediately") {
        return Some(Utc::now());
    }

    // 1. Try standard RFC 3339 / ISO 8601
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }

    // 2. Relative durations (e.g. "in 30 mins", "in 1 hour", "in 45m", "after 2 hours", "+30m", "15 minutes")
    let lower_trim = s.to_lowercase();
    let trimmed = lower_trim
        .strip_prefix("in ")
        .or_else(|| lower_trim.strip_prefix("after "))
        .or_else(|| lower_trim.strip_prefix('+'))
        .unwrap_or(&lower_trim)
        .trim();

    let mut num_str = String::new();
    let mut unit_str = String::new();
    let mut in_unit = false;
    for c in trimmed.chars() {
        if !in_unit && (c.is_ascii_digit() || c == '.') {
            num_str.push(c);
        } else if !num_str.is_empty() {
            in_unit = true;
            unit_str.push(c);
        }
    }
    let unit_str = unit_str.trim();

    if let Ok(num) = num_str.parse::<f64>() {
        if num.is_finite() && num >= 0.0 {
            match unit_str {
                "m" | "min" | "mins" | "minute" | "minutes" => {
                    return Some(
                        Utc::now() + chrono::Duration::milliseconds((num * 60_000.0) as i64),
                    );
                }
                "h" | "hr" | "hrs" | "hour" | "hours" => {
                    return Some(
                        Utc::now() + chrono::Duration::milliseconds((num * 3_600_000.0) as i64),
                    );
                }
                "d" | "day" | "days" => {
                    return Some(
                        Utc::now() + chrono::Duration::milliseconds((num * 86_400_000.0) as i64),
                    );
                }
                _ => {}
            }
        }
    }

    // 3. Try NaiveDateTime with common date+time formats
    let formats = [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M",
        "%d-%m-%Y %H:%M:%S",
        "%d-%m-%Y %H:%M",
        "%d/%m/%Y %H:%M:%S",
        "%d/%m/%Y %H:%M",
        "%Y/%m/%d %H:%M:%S",
        "%Y/%m/%d %H:%M",
    ];
    for fmt in &formats {
        if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
            return Some(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc));
        }
    }

    // 3. Handle human strings like "5 pm", "5:00 PM", "5 pm today", "5 pm tomorrow"
    let mut lower = s.to_lowercase();
    let mut target_date = Utc::now().date_naive();
    if lower.contains("tomorrow") {
        target_date += chrono::Duration::days(1);
        lower = lower.replace("tomorrow", "");
    }
    if lower.contains("today") {
        lower = lower.replace("today", "");
    }
    if lower.contains("at") {
        lower = lower.replace("at", "");
    }
    let lower = lower.trim().to_string();

    let is_pm = lower.ends_with("pm");
    let is_am = lower.ends_with("am");
    if is_pm || is_am {
        let num_part = lower.trim_end_matches("pm").trim_end_matches("am").trim();
        let parts: Vec<&str> = num_part.split(':').collect();
        if let Ok(mut h) = parts[0].parse::<u32>() {
            let m = if parts.len() > 1 {
                parts[1].parse::<u32>().unwrap_or(0)
            } else {
                0
            };
            if is_pm && h < 12 {
                h += 12;
            }
            if is_am && h == 12 {
                h = 0;
            }
            if let Some(time) = chrono::NaiveTime::from_hms_opt(h, m, 0) {
                let naive_dt = chrono::NaiveDateTime::new(target_date, time);
                return Some(DateTime::<Utc>::from_naive_utc_and_offset(naive_dt, Utc));
            }
        }
    }

    // 4. Try 24-hour time like "17:00" or "17:30:00"
    let parts: Vec<&str> = lower.split(':').collect();
    if parts.len() == 2 || parts.len() == 3 {
        if let (Ok(h), Ok(m)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
            if let Some(time) = chrono::NaiveTime::from_hms_opt(h, m, 0) {
                let naive_dt = chrono::NaiveDateTime::new(target_date, time);
                return Some(DateTime::<Utc>::from_naive_utc_and_offset(naive_dt, Utc));
            }
        }
    }

    // 5. Try date-only (e.g. "2026-11-19")
    if let Ok(date) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let naive_dt = date
            .and_hms_opt(Utc::now().time().hour(), Utc::now().time().minute(), 0)
            .unwrap_or_else(|| date.and_hms_opt(0, 0, 0).unwrap());
        return Some(DateTime::<Utc>::from_naive_utc_and_offset(naive_dt, Utc));
    }

    None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Journey {
    pub id: Uuid,
    pub driver_id: Uuid,
    pub driver_gender: Gender,
    pub origin_name: String,
    pub origin_point: GeoPoint,
    pub destination_name: String,
    pub destination_point: GeoPoint,
    pub corridor_buffer_meters: u32,
    pub departure_time_window_start: DateTime<Utc>,
    pub departure_time_window_end: DateTime<Utc>,
    pub available_seats: u32,
    pub cargo_weight_capacity_kg: f64,
    pub cargo_volume_liters: f64,
    pub flexible_detour_minutes: u32,
    pub suggested_cost_share_currency: String,
    pub suggested_cost_per_seat: u32, // in minor currency units (paise / cents)
    pub status: JourneyStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Journey {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        driver_id: Uuid,
        driver_gender: Gender,
        origin_name: String,
        origin_point: GeoPoint,
        destination_name: String,
        destination_point: GeoPoint,
        departure_start: DateTime<Utc>,
        departure_end: DateTime<Utc>,
        seats: u32,
        cargo_kg: f64,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            driver_id,
            driver_gender,
            origin_name,
            origin_point,
            destination_name,
            destination_point,
            corridor_buffer_meters: 1500,
            departure_time_window_start: departure_start,
            departure_time_window_end: departure_end,
            available_seats: seats,
            cargo_weight_capacity_kg: cargo_kg,
            cargo_volume_liters: cargo_kg * 1.5,
            flexible_detour_minutes: 15,
            suggested_cost_share_currency: "INR".to_string(),
            suggested_cost_per_seat: 8000, // ₹80.00 default for corridor
            status: JourneyStatus::Active,
            created_at: now,
            updated_at: now,
        }
    }

    /// Atomically reserve seats
    pub fn reserve_seats(&mut self, count: u32) -> Result<()> {
        if count == 0 {
            return Err(CoreError::ValidationError(
                "Seats requested must be greater than zero".into(),
            ));
        }
        if self.available_seats < count {
            return Err(CoreError::CapacityExceeded {
                requested: count,
                available: self.available_seats,
            });
        }
        self.available_seats -= count;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Restore seats (e.g. on cancellation)
    pub fn release_seats(&mut self, count: u32) {
        self.available_seats += count;
        self.updated_at = Utc::now();
    }

    /// Atomically reserve cargo capacity (validates both weight and volume)
    pub fn reserve_cargo(&mut self, weight_kg: f64, volume_liters: f64) -> Result<()> {
        if !weight_kg.is_finite()
            || !volume_liters.is_finite()
            || weight_kg < 0.0
            || volume_liters < 0.0
        {
            return Err(CoreError::ValidationError(
                "Cargo weight and volume must be finite non-negative values".into(),
            ));
        }
        if self.cargo_weight_capacity_kg < weight_kg {
            return Err(CoreError::CapacityExceeded {
                requested: weight_kg.ceil() as u32,
                available: self.cargo_weight_capacity_kg as u32,
            });
        }
        if self.cargo_volume_liters < volume_liters {
            return Err(CoreError::CapacityExceeded {
                requested: volume_liters.ceil() as u32,
                available: self.cargo_volume_liters as u32,
            });
        }
        self.cargo_weight_capacity_kg -= weight_kg;
        self.cargo_volume_liters -= volume_liters;
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn release_cargo(&mut self, weight_kg: f64, volume_liters: f64) {
        if weight_kg.is_finite() && weight_kg > 0.0 {
            self.cargo_weight_capacity_kg += weight_kg;
        }
        if volume_liters.is_finite() && volume_liters > 0.0 {
            self.cargo_volume_liters += volume_liters;
        }
        self.updated_at = Utc::now();
    }

    pub fn complete(&mut self) -> Result<()> {
        if self.status != JourneyStatus::Active && self.status != JourneyStatus::InProgress {
            return Err(CoreError::InvalidStateTransition {
                from: format!("{:?}", self.status),
                to: "Completed".into(),
            });
        }
        self.status = JourneyStatus::Completed;
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn cancel(&mut self) -> Result<()> {
        if self.status == JourneyStatus::Completed {
            return Err(CoreError::InvalidStateTransition {
                from: "Completed".into(),
                to: "Cancelled".into(),
            });
        }
        self.status = JourneyStatus::Cancelled;
        self.updated_at = Utc::now();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_landmark_resolution() {
        let cet = resolve_known_landmark("CET Trivandrum").unwrap();
        assert!((cet.latitude - 8.5458).abs() < 0.001);

        let chavadi = resolve_known_landmark("chavadimuk").unwrap();
        assert!((chavadi.latitude - 8.5372).abs() < 0.001);

        let tvm = resolve_known_landmark("tvm railway station").unwrap();
        assert!((tvm.latitude - 8.4875).abs() < 0.001);

        let tvm_plain = resolve_known_landmark("Thiruvananthapuram").unwrap();
        assert!((tvm_plain.latitude - 8.4875).abs() < 0.001);

        let tvm_short = resolve_known_landmark("TVM").unwrap();
        assert!((tvm_short.latitude - 8.4875).abs() < 0.001);

        let ernakulam = resolve_known_landmark("Ernakulam").unwrap();
        assert!((ernakulam.latitude - 9.9938).abs() < 0.001);

        let kochi = resolve_known_landmark("Kochi").unwrap();
        assert!((kochi.latitude - 9.9938).abs() < 0.001);

        let lulu = resolve_known_landmark("Lulu Mall Trivandrum").unwrap();
        assert!((lulu.latitude - 8.5303).abs() < 0.001);

        let airport = resolve_known_landmark("Trivandrum Airport (TRV)").unwrap();
        assert!((airport.latitude - 8.4821).abs() < 0.001);

        let blr = resolve_known_landmark("Kempegowda Airport Bangalore").unwrap();
        assert!((blr.latitude - 13.1986).abs() < 0.001);

        assert!(resolve_known_landmark("Unknown place 12345").is_none());
    }

    #[test]
    fn test_parse_flexible_datetime() {
        assert!(parse_flexible_datetime("now").is_some());
        assert!(parse_flexible_datetime("2026-11-19T17:00:00Z").is_some());
        assert!(parse_flexible_datetime("2026-11-19 17:00:00").is_some());
        assert!(parse_flexible_datetime("2026-11-19 17:00").is_some());
        assert!(parse_flexible_datetime("5:00 PM").is_some());
        assert!(parse_flexible_datetime("5 pm").is_some());
        assert!(parse_flexible_datetime("5 pm today").is_some());
        assert!(parse_flexible_datetime("9 am tomorrow").is_some());
        assert!(parse_flexible_datetime("17:00").is_some());

        // Relative durations
        let in_30m = parse_flexible_datetime("in 30 mins").expect("Should parse 'in 30 mins'");
        let diff_mins = (in_30m - Utc::now()).num_minutes();
        assert!((29..=31).contains(&diff_mins));

        let in_1h = parse_flexible_datetime("in 1 hour").expect("Should parse 'in 1 hour'");
        let diff_hours = (in_1h - Utc::now()).num_minutes();
        assert!((59..=61).contains(&diff_hours));

        assert!(parse_flexible_datetime("in 45m").is_some());
        assert!(parse_flexible_datetime("after 2 hours").is_some());
        assert!(parse_flexible_datetime("+15 mins").is_some());
        assert!(parse_flexible_datetime("20 minutes").is_some());
    }

    #[test]
    fn test_journey_cargo_capacity_and_volume_validation() {
        let mut journey = Journey::new(
            Uuid::new_v4(),
            Gender::Male,
            "Origin".into(),
            GeoPoint::new(12.9, 77.5),
            "Dest".into(),
            GeoPoint::new(12.95, 77.55),
            Utc::now(),
            Utc::now() + chrono::Duration::hours(2),
            3,
            10.0, // 10 kg weight, 15 liters volume (10 * 1.5)
        );

        assert_eq!(journey.cargo_weight_capacity_kg, 10.0);
        assert_eq!(journey.cargo_volume_liters, 15.0);

        // Reject invalid bounds
        assert!(journey.reserve_cargo(-1.0, 5.0).is_err());
        assert!(journey.reserve_cargo(2.0, f64::NAN).is_err());

        // Reject exceeding volume even if weight fits
        assert!(journey.reserve_cargo(5.0, 20.0).is_err());

        // Reject exceeding weight even if volume fits
        assert!(journey.reserve_cargo(12.0, 5.0).is_err());

        // Successful reservation within both capacities
        assert!(journey.reserve_cargo(4.0, 6.0).is_ok());
        assert_eq!(journey.cargo_weight_capacity_kg, 6.0);
        assert_eq!(journey.cargo_volume_liters, 9.0);

        // Release cargo
        journey.release_cargo(4.0, 6.0);
        assert_eq!(journey.cargo_weight_capacity_kg, 10.0);
        assert_eq!(journey.cargo_volume_liters, 15.0);
    }
}
