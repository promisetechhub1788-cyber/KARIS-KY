#![no_main]

use libfuzzer_sys::fuzz_target;

// Fuzz target for the `migrate` entrypoint.
//
// The `migrate` entrypoint accepts a `from_version: u32` that is validated
// against the stored schema version and the current `SCHEMA_VERSION` constant.
// This target feeds arbitrary `from_version` values and asserts that the three
// guard conditions (version mismatch, already current, no migration path)
// always produce typed errors rather than panics.
//
// Out of scope: arbitrary contract state is not fuzzed here.
fuzz_target!(|from_version: u32| {
    // The migrate entrypoint must never panic for any `from_version` value.
    // Each guard condition is expected to return a typed error (or Ok when a
    // migration path exists), never an unwrap/panic.
    let result = escrow::migrate(from_version);

    // A panic would abort the fuzzer before reaching this point. If we get
    // here, the entrypoint returned a typed result as required.
    match result {
        Ok(_) => {}
        Err(_) => {}
    }
});
