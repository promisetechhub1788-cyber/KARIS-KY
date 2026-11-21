//! Input validation module for the escrow contract.
//!
//! Centralized validation for all external inputs (strings, arrays, numeric values).
//! All validation functions return a descriptive [`crate::EscrowError`] on failure.

use crate::{EscrowError, MAX_INVOICE_ID_STRING_LEN};
use soroban_sdk::{Env, String, Symbol, Vec};

/// Validates an invoice ID string before conversion to Symbol.
///
/// # Rules
/// - Length: 1 to [`MAX_INVOICE_ID_STRING_LEN`] (inclusive)
/// - Characters: ASCII alphanumeric (0-9, a-z, A-Z) and underscore only
///
/// # Errors
/// - [`EscrowError::InvoiceIdInvalidLength`] if length is 0 or > [`MAX_INVOICE_ID_STRING_LEN`]
/// - [`EscrowError::InvoiceIdInvalidCharset`] if non-ASCII or invalid characters are found
///
/// # Returns
/// A [`Symbol`] representation of the validated string.
pub fn validate_invoice_id(env: &Env, invoice_id: &String) -> Result<Symbol, EscrowError> {
    let len = invoice_id.len();
    if !(1..=MAX_INVOICE_ID_STRING_LEN).contains(&len) {
        return Err(EscrowError::InvoiceIdInvalidLength);
    }

    let len_u = len as usize;
    let mut buf = [0u8; 32];
    invoice_id.copy_into_slice(&mut buf[..len_u]);

    // Validate charset: alphanumeric + underscore
    for &b in &buf[..len_u] {
        let ok =
            b.is_ascii_uppercase() || b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_';
        if !ok {
            return Err(EscrowError::InvoiceIdInvalidCharset);
        }
    }

    // Convert to UTF-8 string
    let s =
        core::str::from_utf8(&buf[..len_u]).map_err(|_| EscrowError::InvoiceIdInvalidCharset)?;

    Ok(Symbol::new(env, s))
}

/// Validates a string field for maximum byte length.
///
/// # Parameters
/// - `s`: The string to validate
/// - `max_bytes`: Maximum UTF-8 byte length (e.g., 256)
///
/// # Returns
/// `Ok(())` if valid, or the provided error on failure.
pub fn validate_string_max_length(s: &String, max_bytes: u32) -> Result<(), EscrowError> {
    if s.len() > max_bytes {
        Err(EscrowError::InvoiceIdInvalidLength) // Reuse for any string length error
    } else {
        Ok(())
    }
}

/// Validates that a positive amount is within acceptable bounds.
///
/// # Parameters
/// - `amount`: The amount to validate
///
/// # Errors
/// - [`EscrowError::FundingAmountNotPositive`] if amount <= 0
/// - [`EscrowError::AmountMustBePositive`] for init-time amount validation
///
/// # Returns
/// `Ok(())` if valid.
pub fn validate_positive_amount(amount: i128) -> Result<(), EscrowError> {
    if amount > 0 {
        Ok(())
    } else {
        Err(EscrowError::FundingAmountNotPositive)
    }
}

/// Validates a percentage (basis points) value.
///
/// # Parameters
/// - `bps`: Basis points (0-10000 = 0%-100%)
///
/// # Errors
/// - [`EscrowError::YieldBpsOutOfRange`] if bps < 0 or bps > 10000
///
/// # Returns
/// `Ok(())` if valid.
pub fn validate_yield_bps(bps: i64) -> Result<(), EscrowError> {
    if (0..=10_000).contains(&bps) {
        Ok(())
    } else {
        Err(EscrowError::YieldBpsOutOfRange)
    }
}

/// Validates an array/batch is not empty and within size bounds.
///
/// # Parameters
/// - `len`: Current number of items
/// - `max_len`: Maximum allowed items
/// - `empty_error`: Error to return if empty
/// - `full_error`: Error to return if exceeds max
///
/// # Returns
/// `Ok(())` if valid.
pub fn validate_batch_size(
    len: u32,
    max_len: u32,
    empty_error: EscrowError,
    full_error: EscrowError,
) -> Result<(), EscrowError> {
    if len == 0 {
        Err(empty_error)
    } else if len > max_len {
        Err(full_error)
    } else {
        Ok(())
    }
}

/// Validates that a numeric value is positive.
///
/// # Parameters
/// - `value`: The value to validate
/// - `error`: Error to return if value <= 0
///
/// # Returns
/// `Ok(())` if valid.
pub fn validate_positive_value(value: i128, error: EscrowError) -> Result<(), EscrowError> {
    if value > 0 {
        Ok(())
    } else {
        Err(error)
    }
}

/// Validates that an upper bound value is not zero.
///
/// # Parameters
/// - `value`: The value to validate
/// - `error`: Error to return if value == 0
///
/// # Returns
/// `Ok(())` if valid.
pub fn validate_nonzero(value: u32, error: EscrowError) -> Result<(), EscrowError> {
    if value > 0 {
        Ok(())
    } else {
        Err(error)
    }
}

/// Validates that value A does not exceed value B.
///
/// # Parameters
/// - `value`: The value being checked
/// - `max`: The maximum allowed value
/// - `error`: Error to return if value > max
///
/// # Returns
/// `Ok(())` if valid.
pub fn validate_not_exceeds(value: i128, max: i128, error: EscrowError) -> Result<(), EscrowError> {
    if value <= max {
        Ok(())
    } else {
        Err(error)
    }
}

/// Validates that `new_value` is strictly lower than `current_value`.
///
/// Used for monotonic decrements (e.g., lowering investor caps).
///
/// # Parameters
/// - `new_value`: The proposed new value
/// - `current_value`: The existing value
/// - `error`: Error to return if new_value >= current_value
///
/// # Returns
/// `Ok(())` if valid.
pub fn validate_strictly_lower(
    new_value: u32,
    current_value: u32,
    error: EscrowError,
) -> Result<(), EscrowError> {
    if new_value < current_value {
        Ok(())
    } else {
        Err(error)
    }
}

/// Validates that a string is not empty.
///
/// # Parameters
/// - `s`: The string to validate
/// - `error`: Error to return if empty
///
/// # Returns
/// `Ok(())` if valid.
pub fn validate_string_not_empty(s: &String, error: EscrowError) -> Result<(), EscrowError> {
    if s.len() > 0 {
        Ok(())
    } else {
        Err(error)
    }
}

/// Validates that a value is within a numeric range (inclusive).
///
/// # Parameters
/// - `value`: The value to validate
/// - `min`: Minimum allowed value (inclusive)
/// - `max`: Maximum allowed value (inclusive)
/// - `error`: Error to return if outside range
///
/// # Returns
/// `Ok(())` if valid.
pub fn validate_range<T: PartialOrd>(
    value: T,
    min: T,
    max: T,
    error: EscrowError,
) -> Result<(), EscrowError> {
    if value >= min && value <= max {
        Ok(())
    } else {
        Err(error)
    }
}

/// Validates that two addresses are different.
///
/// Used to prevent self-assignment errors (e.g., proposing self as new admin).
///
/// # Parameters
/// - `addr1`: First address
/// - `addr2`: Second address
/// - `error`: Error to return if addresses are equal
///
/// # Returns
/// `Ok(())` if addresses differ.
pub fn validate_addresses_differ(
    addr1: &soroban_sdk::Address,
    addr2: &soroban_sdk::Address,
    error: EscrowError,
) -> Result<(), EscrowError> {
    if addr1 != addr2 {
        Ok(())
    } else {
        Err(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_invoice_id_valid() {
        // Valid invoice IDs cannot be tested without Env context; tested via contract tests
    }

    #[test]
    fn test_validate_yield_bps() {
        assert!(validate_yield_bps(0).is_ok());
        assert!(validate_yield_bps(5000).is_ok());
        assert!(validate_yield_bps(10000).is_ok());
        assert!(validate_yield_bps(-1).is_err());
        assert!(validate_yield_bps(10001).is_err());
    }

    #[test]
    fn test_validate_positive_amount() {
        assert!(validate_positive_amount(1).is_ok());
        assert!(validate_positive_amount(1_000_000).is_ok());
        assert!(validate_positive_amount(0).is_err());
        assert!(validate_positive_amount(-1).is_err());
    }

    #[test]
    fn test_validate_batch_size() {
        assert!(validate_batch_size(
            1,
            10,
            EscrowError::FundingBatchEmpty,
            EscrowError::FundingBatchTooLarge
        )
        .is_ok());

        assert!(validate_batch_size(
            10,
            10,
            EscrowError::FundingBatchEmpty,
            EscrowError::FundingBatchTooLarge
        )
        .is_ok());

        assert!(validate_batch_size(
            0,
            10,
            EscrowError::FundingBatchEmpty,
            EscrowError::FundingBatchTooLarge
        )
        .is_err());

        assert!(validate_batch_size(
            11,
            10,
            EscrowError::FundingBatchEmpty,
            EscrowError::FundingBatchTooLarge
        )
        .is_err());
    }

    #[test]
    fn test_validate_positive_value() {
        assert!(validate_positive_value(1, EscrowError::MinContributionNotPositive).is_ok());
        assert!(validate_positive_value(0, EscrowError::MinContributionNotPositive).is_err());
        assert!(validate_positive_value(-1, EscrowError::MinContributionNotPositive).is_err());
    }

    #[test]
    fn test_validate_not_exceeds() {
        assert!(validate_not_exceeds(5, 10, EscrowError::YieldBpsOutOfRange).is_ok());
        assert!(validate_not_exceeds(10, 10, EscrowError::YieldBpsOutOfRange).is_ok());
        assert!(validate_not_exceeds(11, 10, EscrowError::YieldBpsOutOfRange).is_err());
    }

    #[test]
    fn test_validate_strictly_lower() {
        assert!(validate_strictly_lower(5, 10, EscrowError::NewCapNotLower).is_ok());
        assert!(validate_strictly_lower(10, 10, EscrowError::NewCapNotLower).is_err());
        assert!(validate_strictly_lower(11, 10, EscrowError::NewCapNotLower).is_err());
    }

    #[test]
    fn test_validate_range() {
        assert!(validate_range(5, 0, 10, EscrowError::YieldBpsOutOfRange).is_ok());
        assert!(validate_range(0, 0, 10, EscrowError::YieldBpsOutOfRange).is_ok());
        assert!(validate_range(10, 0, 10, EscrowError::YieldBpsOutOfRange).is_ok());
        assert!(validate_range(-1, 0, 10, EscrowError::YieldBpsOutOfRange).is_err());
        assert!(validate_range(11, 0, 10, EscrowError::YieldBpsOutOfRange).is_err());
    }
}
