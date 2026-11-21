# Escrow Fund Parameters

This document describes the configurable parameters of the escrow fund contract,
including funding rate limiting controls used by operators.

## Funding Rate Limiting

The escrow fund enforces a funding rate limit to protect the fund from
unexpectedly large or rapid funding operations. Three entry points control this
behaviour:

- `set_max_funding_rate` — configure the maximum allowed funding rate.
- `is_funding_paused` — query whether funding is currently paused.
- `resume_funding` — clear an automatic pause and allow funding again.

### `set_max_funding_rate`

Sets the maximum funding rate the fund will accept. The rate is measured as the
amount of funding accepted per unit of time (for example, per ledger/block
window). When a funding operation would push the observed rate above this limit,
the fund automatically pauses further funding.

Example:

```
// Allow at most 1,000,000 units of funding per window.
set_max_funding_rate(1_000_000);
```

Setting the value to `0` disables rate limiting (no automatic pause from rate
breaches). Choose a value that reflects the fund's expected inflow and risk
tolerance.

### `is_funding_paused`

Returns whether funding is currently paused. Funding becomes paused when the
observed funding rate exceeds the configured `set_max_funding_rate` limit. Use
this to check state before attempting funding operations or when monitoring the
fund.

Example:

```
if is_funding_paused() {
    // Investigate the cause before resuming.
}
```

### `resume_funding`

Clears a pause and allows funding to proceed again. This is an operator action:
resume only after confirming the cause of the pause (for example, an unexpected
inflow spike) has been addressed. After resuming, the rate limiter continues to
apply against the configured `set_max_funding_rate`.

Example:

```
// After investigating and resolving the rate breach.
resume_funding();
```

### Operational Notes

- A pause is automatic: it is triggered by the rate limiter, not by a manual
  pause flag.
- `resume_funding` does not change the configured limit; adjust
  `set_max_funding_rate` separately if the limit itself needs to change.
- Monitor `is_funding_paused` as part of routine fund health checks.
