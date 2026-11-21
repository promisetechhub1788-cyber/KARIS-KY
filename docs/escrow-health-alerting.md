# Escrow Health Warning Alerts

The backend accepts decoded `EscrowHealthWarning` events at
`POST /api/events/escrow-health-warning`. Configure a Soroban event indexer to
watch the escrow contract and forward this event (identified by its `hlth_wrn`
event topic) as JSON. The `warning_type` field must be an unsigned 32-bit
integer; each non-zero code triggers an alert, while code `0` is acknowledged
without an alert.

Example request:

```json
{
  "invoice_id": "invoice-123",
  "warning_type": 4001,
  "funded_amount": "250000",
  "funding_target": "1000000",
  "funded_ratio_bps": 2500,
  "time_to_maturity_secs": 3600,
  "recorded_at_ledger_timestamp": 1730000000
}
```

By default, the backend writes a structured alert containing the event name,
warning code, and invoice ID to its error log. To additionally forward alerts
to an on-call or messaging service, set
`ESCROW_HEALTH_ALERT_WEBHOOK_URL` to that service's HTTPS webhook URL before
starting the backend. The backend sends a JSON `POST` containing `event`,
`warning_code`, and `invoice_id`; a non-success webhook response is logged and
the event endpoint returns HTTP 502 so the indexer can retry. Keep webhook URLs
secret and configure the indexer to retry failed deliveries. The event receiver
has no authentication of its own; keep it on a trusted network or restrict
access to the indexer at the reverse proxy/firewall.

Run the backend with `node backend/server.js`. The receiver is intentionally
vendor-neutral; event decoding and subscription are responsibilities of the
configured indexer.
