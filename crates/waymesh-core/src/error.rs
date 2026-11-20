use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Invalid cryptographic signature")]
    InvalidSignature,

    #[error("Cryptographic key error: {0}")]
    KeyError(String),

    #[error("OTP verification failed: incorrect or expired token")]
    OtpMismatch,

    #[error("OTP verification locked: maximum attempt limit exceeded")]
    OtpAttemptsExceeded,

    #[error("Invalid geographic coordinates: latitude {lat}, longitude {lng}")]
    InvalidCoordinates { lat: f64, lng: f64 },

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Invalid state transition from {from} to {to}")]
    InvalidStateTransition { from: String, to: String },

    #[error("Capacity exceeded: requested {requested}, available {available}")]
    CapacityExceeded { requested: u32, available: u32 },

    #[error("Gender incompatibility in ride matching: cannot match male and female")]
    GenderIncompatible,

    #[error("Self-dealing prohibited: passenger and driver (or sender and carrier) cannot be the same entity")]
    SelfDealingProhibited,

    #[error("Domain restriction: email '{0}' is not authorized. Access is restricted to verified '@cet.ac.in' student/faculty accounts")]
    UnauthorizedDomain(String),

    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("Unauthorized action: {0}")]
    Unauthorized(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;
