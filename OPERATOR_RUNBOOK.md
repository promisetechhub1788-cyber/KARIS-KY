# Operator Runbook

Operational procedures and remediation guidance for running the service in production.

## State Inconsistency Detection

The `detect_state_inconsistencies` routine scans persisted state and reports findings
where the stored data does not match the expected invariants. Each finding type below
includes what it means and the operator action required to remediate it.

### Remediation Table

| Finding type | What it means | Operator remediation |
| --- | --- | --- |
| `orphaned_record` | A record references a parent/owner that no longer exists, leaving it unreachable from normal flows. | Identify the orphan via the finding's identifiers, confirm the parent is truly gone, then archive or delete the orphaned record per the data-retention policy. If the parent should exist, restore it from backup before removing the child. |
| `missing_reference` | A record points to a related entity that cannot be found, so lookups and joins will fail. | Locate the missing referenced entity. If it was deleted intentionally, update the referencing record to clear or repoint the reference. If it was lost, restore it from backup and re-run detection to confirm the finding clears. |
| `duplicate_record` | Two or more records represent the same logical entity, which can cause double-processing or inconsistent reads. | Determine the canonical record, merge any unique data into it, then remove the duplicates. Re-run detection to confirm only one record remains. |
| `stale_status` | A record's status has not advanced within the expected window and no longer reflects reality. | Inspect the record's history and any blocking dependencies. Advance the status to the correct value, or reset it if the transition was spurious. Investigate the upstream process that failed to update it. |
| `inconsistent_balance` | A stored balance does not match the sum of its underlying entries/transactions. | Recompute the balance from the source entries. If the recomputed value is authoritative, correct the stored balance. If entries are missing, restore them from backup first, then recompute. |
| `dangling_lock` | A lock or reservation is held by a process/owner that no longer exists, blocking other operations. | Confirm the owning process is gone, then release the lock. If the owner is still active, do not force-release; investigate why it is not progressing. |
| `timestamp_anomaly` | A record has timestamps that are out of order or in the future, indicating clock or write-path issues. | Verify the host clock (NTP) and the write path. Correct the affected timestamps to the true event times where known; if unknown, flag the record for manual review rather than guessing. |
| `schema_mismatch` | A persisted record does not conform to the current expected schema version. | Run the appropriate migration for the record's version. If no migration exists, escalate to engineering with the finding details before making manual edits. |

### General Procedure

1. Capture the full finding output (type, identifiers, timestamps) before making changes.
2. Take a backup/snapshot of the affected data.
3. Apply the remediation for the specific finding type from the table above.
4. Re-run `detect_state_inconsistencies` to confirm the finding no longer appears.
5. If a finding recurs after remediation, escalate to engineering — it likely indicates an upstream bug rather than a one-off data issue.

## Pre-Flight Checklist Validation

`scripts/pre-flight-checklist.sh` must stay in sync with the actual deploy steps. CI runs a
validation step that checks the checklist's references still exist, so a stale checklist
fails the build instead of misleading an operator during a deploy.

### What the check does

- Extracts every file path referenced by the checklist script and verifies each one exists
  in the repository.
- Extracts every `stellar contract invoke` method referenced by the checklist and verifies
  the method is still present in the contract source.
- Fails the CI job (non-zero exit) if any referenced file or method is missing.

### Updating the checklist

When a contract entrypoint is renamed or removed, or a referenced file is moved or deleted:

1. Update `scripts/pre-flight-checklist.sh` so it references the current file paths and
   `stellar contract invoke` methods.
2. Run the validation locally (the same command CI uses) to confirm it passes.
3. Commit the checklist change together with the entrypoint/file change that caused it.

If the validation fails in CI, treat it as a signal that the checklist is stale — fix the
references rather than disabling the check.
