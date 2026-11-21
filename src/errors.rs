//! Escrow error types and error code numbering scheme.
//!
//! # Error code numbering scheme
//!
//! Error codes are grouped into reserved numeric ranges so that related
//! failures stay together and new codes can be added without collisions.
//! The ranges currently in use are:
//!
//! | Range   | Area                          | Next free code |
//! |---------|-------------------------------|----------------|
//! | 1–13    | Initialization                | 14             |
//! | 20–29   | Configuration                 | 21             |
//! | 30–39   | Sweep                         | 31             |
//! | 100–119 | Funding                       | 101            |
//! | 120–139 | Settlement                    | 121            |
//! | 140–159 | Dispute                       | 141            |
//! | 160–179 | Refund                        | 161            |
//! | 180–199 | Validation                    | 184            |
//!
//! When adding a new error, pick the next free code in the matching range
//! and update the "Next free code" column above. Do not renumber existing
//! codes: they are part of the public contract.

use std::fmt;

/// Errors returned by the escrow contract.
///
/// See the module-level documentation for the error code numbering scheme
/// and the reserved ranges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscrowError {
    // Initialization (1–13)
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidAdmin = 3,
    InvalidToken = 4,
    InvalidEscrowId = 5,
    InvalidAmount = 6,
    InvalidDeadline = 7,
    InvalidBuyer = 8,
    InvalidSeller = 9,
    InvalidArbiter = 10,
    InvalidFeeBps = 11,
    InvalidState = 12,
    Unauthorized = 13,

    // Configuration (20–29)
    ConfigNotFound = 20,

    // Sweep (30–39)
    SweepNotAllowed = 30,

    // Funding (100–119)
    FundingFailed = 100,

    // Settlement (120–139)
    SettlementFailed = 120,

    // Dispute (140–159)
    DisputeFailed = 140,

    // Refund (160–179)
    RefundFailed = 160,

    // Validation (180–199)
    InvalidInput = 180,
    InvalidSignature = 181,
    InvalidNonce = 182,
    InvalidProof = 183,
}

impl EscrowError {
    /// Returns the numeric error code for this error.
    pub fn code(&self) -> u32 {
        *self as u32
    }
}

impl fmt::Display for EscrowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "escrow error {}", self.code())
    }
}

impl std::error::Error for EscrowError {}
