use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Gender {
    #[default]
    Male,
    Female,
    Other,
}

impl std::fmt::Display for Gender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Gender::Male => write!(f, "male"),
            Gender::Female => write!(f, "female"),
            Gender::Other => write!(f, "other"),
        }
    }
}

impl FromStr for Gender {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "male" | "m" => Ok(Gender::Male),
            "female" | "f" => Ok(Gender::Female),
            "other" => Ok(Gender::Other),
            unknown => Err(format!("Unknown gender: {}", unknown)),
        }
    }
}

impl Gender {
    /// In passenger ride matching, male and female participants must not be matched.
    pub fn is_compatible_for_ride(&self, other: &Gender) -> bool {
        !((*self == Gender::Male && *other == Gender::Female)
            || (*self == Gender::Female && *other == Gender::Male))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IdentityRole {
    User,
    Driver,
    Carrier,
    NodeOperator,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub id: Uuid,
    pub public_key_ed25519: String,
    pub display_name: String,
    pub gender: Gender,
    pub email: Option<String>,
    pub oauth_provider: Option<String>,
    pub oauth_sub: Option<String>,
    pub is_verified: bool,
    pub phone_hash: Option<String>,
    pub reputation_score: f32,
    pub total_trips_completed: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Identity {
    pub const ALLOWED_CAMPUS_DOMAIN: &'static str = "@cet.ac.in";

    pub fn new(public_key_ed25519: String, display_name: String, gender: Gender) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            public_key_ed25519,
            display_name,
            gender,
            email: None,
            oauth_provider: None,
            oauth_sub: None,
            is_verified: false,
            phone_hash: None,
            reputation_score: 5.0,
            total_trips_completed: 0,
            created_at: now,
            updated_at: now,
        }
    }

    /// Construct a verified identity authenticated via OAuth (restricted to CET campus domain)
    pub fn with_oauth(
        public_key_ed25519: String,
        display_name: String,
        gender: Gender,
        email: String,
        oauth_provider: String,
        oauth_sub: String,
    ) -> crate::Result<Self> {
        let email_trimmed = email.trim().to_lowercase();
        if !email_trimmed.ends_with(Self::ALLOWED_CAMPUS_DOMAIN) {
            return Err(crate::CoreError::UnauthorizedDomain(email));
        }
        let now = Utc::now();
        Ok(Self {
            id: Uuid::new_v4(),
            public_key_ed25519,
            display_name,
            gender,
            email: Some(email_trimmed),
            oauth_provider: Some(oauth_provider),
            oauth_sub: Some(oauth_sub),
            is_verified: true,
            phone_hash: None,
            reputation_score: 5.0,
            total_trips_completed: 0,
            created_at: now,
            updated_at: now,
        })
    }

    /// Check if identity is verified with a valid CET institutional email
    pub fn is_cet_verified(&self) -> bool {
        self.is_verified
            && self
                .email
                .as_ref()
                .map(|e| e.to_lowercase().ends_with(Self::ALLOWED_CAMPUS_DOMAIN))
                .unwrap_or(false)
    }

    pub fn increment_trips(&mut self) {
        self.total_trips_completed += 1;
        self.updated_at = Utc::now();
    }
}
