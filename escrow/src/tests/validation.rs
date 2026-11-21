//! Comprehensive input validation tests for the escrow contract.
//!
//! Tests boundary cases, invalid inputs, and edge conditions across all validation rules.

#[cfg(test)]
mod validation_tests {
    use crate::{
        validation::*, EscrowError, LiquifactEscrow, LiquifactEscrowClient, MAX_FUND_BATCH,
        MAX_INVESTOR_ALLOWLIST_BATCH, SCHEMA_VERSION,
    };
    use soroban_sdk::{
        symbol_short,
        testutils::{Address as _, Ledger as _},
        token::Client as TokenClient,
        Address, Env, String,
    };

    fn setup_env() -> Env {
        Env::default()
    }

    fn setup_token(env: &Env, admin: &Address) -> Address {
        let token = Address::generate(env);
        let token_client = TokenClient::new(env, &token);
        // Setup token balances as needed in tests
        token_client.initialize(
            &Address::generate(env),
            &7u32,
            &"Test Token".into(),
            &"TST".into(),
        );
        token
    }

    /// Test invoice ID validation: valid cases
    #[test]
    fn test_invoice_id_valid_cases() {
        let env = setup_env();

        // Single character
        let valid_ids = vec!["a", "Z", "0", "_", "invoice_123", "INV_001", "test_id"];

        for id_str in valid_ids {
            let invoice_id: String = id_str.into();
            let result = validate_invoice_id(&env, &invoice_id);
            assert!(result.is_ok(), "Failed for: {}", id_str);
        }
    }

    /// Test invoice ID validation: length boundary cases
    #[test]
    fn test_invoice_id_length_boundary() {
        let env = setup_env();

        // Empty string (len = 0)
        let empty: String = "".into();
        assert!(
            validate_invoice_id(&env, &empty).is_err(),
            "Empty string should fail"
        );

        // Max length (32 chars)
        let max_valid: String = "a".repeat(32).into();
        assert!(
            validate_invoice_id(&env, &max_valid).is_ok(),
            "Max length should pass"
        );

        // Over max length (33 chars)
        let over_max: String = "a".repeat(33).into();
        assert!(
            validate_invoice_id(&env, &over_max).is_err(),
            "Over max length should fail"
        );
    }

    /// Test invoice ID validation: invalid characters
    #[test]
    fn test_invoice_id_invalid_chars() {
        let env = setup_env();

        let invalid_ids = vec![
            "inv-id",  // hyphen
            "inv.id",  // dot
            "inv@id",  // at symbol
            "inv id",  // space
            "inv\tid", // tab
            "inv#id",  // hash
            "inv$id",  // dollar
        ];

        for id_str in invalid_ids {
            let invoice_id: String = id_str.into();
            let result = validate_invoice_id(&env, &invoice_id);
            assert!(result.is_err(), "Invalid chars should fail for: {}", id_str);
        }
    }

    /// Test positive amount validation
    #[test]
    fn test_positive_amount_boundary() {
        // Zero is invalid
        assert!(validate_positive_amount(0).is_err());

        // Negative is invalid
        assert!(validate_positive_amount(-1).is_err());
        assert!(validate_positive_amount(i128::MIN).is_err());

        // Positive is valid
        assert!(validate_positive_amount(1).is_ok());
        assert!(validate_positive_amount(1_000_000).is_ok());
        assert!(validate_positive_amount(i128::MAX).is_ok());
    }

    /// Test yield basis points validation
    #[test]
    fn test_yield_bps_boundary() {
        // Valid: 0 to 10000
        assert!(validate_yield_bps(0).is_ok());
        assert!(validate_yield_bps(5000).is_ok());
        assert!(validate_yield_bps(10000).is_ok());

        // Invalid: outside range
        assert!(validate_yield_bps(-1).is_err());
        assert!(validate_yield_bps(10001).is_err());
        assert!(validate_yield_bps(i64::MAX).is_err());
        assert!(validate_yield_bps(i64::MIN).is_err());
    }

    /// Test batch size validation
    #[test]
    fn test_batch_size_empty() {
        let result = validate_batch_size(
            0,
            10,
            EscrowError::FundingBatchEmpty,
            EscrowError::FundingBatchTooLarge,
        );
        assert_eq!(result, Err(EscrowError::FundingBatchEmpty));
    }

    /// Test batch size validation: at capacity
    #[test]
    fn test_batch_size_at_capacity() {
        let result = validate_batch_size(
            MAX_FUND_BATCH,
            MAX_FUND_BATCH,
            EscrowError::FundingBatchEmpty,
            EscrowError::FundingBatchTooLarge,
        );
        assert!(result.is_ok());
    }

    /// Test batch size validation: exceeds capacity
    #[test]
    fn test_batch_size_exceeds_capacity() {
        let result = validate_batch_size(
            MAX_FUND_BATCH + 1,
            MAX_FUND_BATCH,
            EscrowError::FundingBatchEmpty,
            EscrowError::FundingBatchTooLarge,
        );
        assert_eq!(result, Err(EscrowError::FundingBatchTooLarge));
    }

    /// Test batch size validation: valid range
    #[test]
    fn test_batch_size_valid_range() {
        for size in 1..=MAX_FUND_BATCH {
            let result = validate_batch_size(
                size,
                MAX_FUND_BATCH,
                EscrowError::FundingBatchEmpty,
                EscrowError::FundingBatchTooLarge,
            );
            assert!(result.is_ok(), "Size {} should be valid", size);
        }
    }

    /// Test value not exceeds validation
    #[test]
    fn test_value_not_exceeds() {
        assert!(validate_not_exceeds(5, 10, EscrowError::YieldBpsOutOfRange).is_ok());
        assert!(validate_not_exceeds(10, 10, EscrowError::YieldBpsOutOfRange).is_ok());
        assert!(validate_not_exceeds(11, 10, EscrowError::YieldBpsOutOfRange).is_err());
        assert!(
            validate_not_exceeds(i128::MAX, i128::MAX, EscrowError::YieldBpsOutOfRange).is_ok()
        );
        assert!(
            validate_not_exceeds(i128::MAX, i128::MAX - 1, EscrowError::YieldBpsOutOfRange)
                .is_err()
        );
    }

    /// Test strictly lower validation (monotonic decrements)
    #[test]
    fn test_strictly_lower() {
        assert!(validate_strictly_lower(5, 10, EscrowError::NewCapNotLower).is_ok());
        assert!(validate_strictly_lower(1, 10, EscrowError::NewCapNotLower).is_ok());
        assert!(validate_strictly_lower(0, 1, EscrowError::NewCapNotLower).is_ok());

        // Equal should fail
        assert!(validate_strictly_lower(10, 10, EscrowError::NewCapNotLower).is_err());

        // Higher should fail
        assert!(validate_strictly_lower(11, 10, EscrowError::NewCapNotLower).is_err());
    }

    /// Test numeric range validation
    #[test]
    fn test_range_validation() {
        // Valid: within range
        assert!(validate_range(5, 0, 10, EscrowError::YieldBpsOutOfRange).is_ok());
        assert!(validate_range(0, 0, 10, EscrowError::YieldBpsOutOfRange).is_ok());
        assert!(validate_range(10, 0, 10, EscrowError::YieldBpsOutOfRange).is_ok());

        // Invalid: below range
        assert!(validate_range(-1, 0, 10, EscrowError::YieldBpsOutOfRange).is_err());

        // Invalid: above range
        assert!(validate_range(11, 0, 10, EscrowError::YieldBpsOutOfRange).is_err());
    }

    /// Test positive value validation
    #[test]
    fn test_positive_value() {
        assert!(validate_positive_value(1, EscrowError::MinContributionNotPositive).is_ok());
        assert!(validate_positive_value(1000, EscrowError::MinContributionNotPositive).is_ok());
        assert!(
            validate_positive_value(i128::MAX, EscrowError::MinContributionNotPositive).is_ok()
        );

        assert!(validate_positive_value(0, EscrowError::MinContributionNotPositive).is_err());
        assert!(validate_positive_value(-1, EscrowError::MinContributionNotPositive).is_err());
        assert!(
            validate_positive_value(i128::MIN, EscrowError::MinContributionNotPositive).is_err()
        );
    }

    /// Test nonzero validation
    #[test]
    fn test_nonzero() {
        assert!(validate_nonzero(1, EscrowError::MaxUniqueInvestorsNotPositive).is_ok());
        assert!(validate_nonzero(u32::MAX, EscrowError::MaxUniqueInvestorsNotPositive).is_ok());

        assert!(validate_nonzero(0, EscrowError::MaxUniqueInvestorsNotPositive).is_err());
    }

    /// Test string max length validation
    #[test]
    fn test_string_max_length() {
        let short: String = "test".into();
        assert!(validate_string_max_length(&short, 100).is_ok());

        let exact: String = "a".repeat(32).into();
        assert!(validate_string_max_length(&exact, 32).is_ok());

        let over: String = "a".repeat(33).into();
        assert!(validate_string_max_length(&over, 32).is_err());
    }

    /// Test string not empty validation
    #[test]
    fn test_string_not_empty() {
        let env = setup_env();
        let empty: String = "".into();
        let not_empty: String = "test".into();

        let empty_result = validate_string_not_empty(&empty, EscrowError::CollateralAssetEmpty);
        let not_empty_result =
            validate_string_not_empty(&not_empty, EscrowError::CollateralAssetEmpty);

        assert!(empty_result.is_err());
        assert!(not_empty_result.is_ok());
    }

    /// Test addresses differ validation
    #[test]
    fn test_addresses_differ() {
        let env = setup_env();
        let addr1 = Address::generate(&env);
        let addr2 = Address::generate(&env);

        // Different addresses should pass
        assert!(
            validate_addresses_differ(&addr1, &addr2, EscrowError::NewSmeSameAsCurrent).is_ok()
        );

        // Same address should fail
        assert!(
            validate_addresses_differ(&addr1, &addr1, EscrowError::NewSmeSameAsCurrent).is_err()
        );
    }

    /// Integration test: invalid init parameters caught by validation
    #[test]
    fn test_init_invalid_amount() {
        let env = setup_env();
        let admin = Address::generate(&env);
        let token = setup_token(&env, &admin);
        let treasury = Address::generate(&env);
        let sme = Address::generate(&env);

        let client = LiquifactEscrowClient::new(&env, &Address::generate(&env));

        // Zero amount should be rejected
        admin.mock_all_auths();
        let result = client.try_init(
            &admin,
            &"valid_id".into(),
            &sme,
            &0,    // Invalid: zero amount
            &5000, // 50% yield
            &0,
            &token,
            &None,
            &treasury,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        );

        // Should fail with AmountMustBePositive
        assert!(result.is_err());
    }

    /// Integration test: invalid yield_bps caught by validation
    #[test]
    fn test_init_invalid_yield_bps() {
        let env = setup_env();
        let admin = Address::generate(&env);
        let token = setup_token(&env, &admin);
        let treasury = Address::generate(&env);
        let sme = Address::generate(&env);

        let client = LiquifactEscrowClient::new(&env, &Address::generate(&env));

        // Out of range yield_bps should be rejected
        admin.mock_all_auths();
        let result = client.try_init(
            &admin,
            &"valid_id".into(),
            &sme,
            &1000,
            &10001, // Invalid: exceeds 10000
            &0,
            &token,
            &None,
            &treasury,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        );

        // Should fail with YieldBpsOutOfRange
        assert!(result.is_err());
    }

    /// Integration test: invalid invoice_id caught by validation
    #[test]
    fn test_init_invalid_invoice_id() {
        let env = setup_env();
        let admin = Address::generate(&env);
        let token = setup_token(&env, &admin);
        let treasury = Address::generate(&env);
        let sme = Address::generate(&env);

        let client = LiquifactEscrowClient::new(&env, &Address::generate(&env));

        // Empty invoice_id should be rejected
        admin.mock_all_auths();
        let result = client.try_init(
            &admin,
            &"".into(), // Invalid: empty
            &sme,
            &1000,
            &5000,
            &0,
            &token,
            &None,
            &treasury,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        );

        // Should fail with InvoiceIdInvalidLength
        assert!(result.is_err());
    }

    /// Test: min_contribution must be positive if set
    #[test]
    fn test_init_invalid_min_contribution() {
        let env = setup_env();
        let admin = Address::generate(&env);
        let token = setup_token(&env, &admin);
        let treasury = Address::generate(&env);
        let sme = Address::generate(&env);

        let client = LiquifactEscrowClient::new(&env, &Address::generate(&env));

        // Non-positive min_contribution should be rejected
        admin.mock_all_auths();
        let result = client.try_init(
            &admin,
            &"valid_id".into(),
            &sme,
            &1000,
            &5000,
            &0,
            &token,
            &None,
            &treasury,
            &None,
            &Some(0), // Invalid: zero min_contribution
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        );

        // Should fail with MinContributionNotPositive
        assert!(result.is_err());
    }

    /// Test: min_contribution must not exceed amount
    #[test]
    fn test_init_min_contribution_exceeds_target() {
        let env = setup_env();
        let admin = Address::generate(&env);
        let token = setup_token(&env, &admin);
        let treasury = Address::generate(&env);
        let sme = Address::generate(&env);

        let client = LiquifactEscrowClient::new(&env, &Address::generate(&env));

        // min_contribution > amount should be rejected
        admin.mock_all_auths();
        let result = client.try_init(
            &admin,
            &"valid_id".into(),
            &sme,
            &1000,
            &5000,
            &0,
            &token,
            &None,
            &treasury,
            &None,
            &Some(2000), // Invalid: exceeds target of 1000
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        );

        // Should fail with MinContributionExceedsAmount
        assert!(result.is_err());
    }
}
