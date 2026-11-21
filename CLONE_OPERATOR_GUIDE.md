# Clone Settled Escrow - Operator Guide

## Quick Reference

### When to Use

Clone a settled escrow when you need to create a new invoice for the **same SME** with:
- Same yield structure (base yield + optional tiers)
- Same maturity timeline
- Same investor constraints (caps, minimum contribution)
- Same governance rules (legal hold delays)
- Different invoice amount or ID

### When NOT to Use

Do NOT clone if you need:
- Different SME address → Create new escrow via `init`
- Different token → Create new escrow via `init`
- Different admin → Create new escrow via `init`
- Modify any immutable configuration → Create new escrow via `init`

### Prerequisites

1. **Template escrow must be settled**
   - Status must be exactly 2 (settled)
   - Cannot clone open, funded, withdrawn, or cancelled escrows
   
2. **Admin must have authority**
   - Only the template escrow's admin can call clone
   - Must provide authorization signature

3. **Valid parameters**
   - `new_invoice_id`: 1-32 ASCII alphanumeric + underscore only
   - `new_amount`: Must be positive (> 0)

## CLI Usage (Stellar/Soroban)

### Step 1: Verify Template Escrow is Settled

```bash
stellar contract invoke \
  --network testnet \
  --source ADMIN_KEY \
  --account ADMIN_ACCOUNT \
  -- get_escrow_summary
```

Check the response: `"status": 2` = settled ✓

### Step 2: Clone the Escrow

```bash
stellar contract invoke \
  --network testnet \
  --source ADMIN_KEY \
  --account ADMIN_ACCOUNT \
  -- clone_settled_escrow \
  --template_env <TEMPLATE_CONTRACT_ID> \
  --new_invoice_id "INV_2025_001" \
  --new_amount 750000000000  # 7.5M (check your token decimals)
```

### Step 3: Verify Clone Success

Look for `EscrowCloned` event in the transaction receipt:

```json
{
  "name": "escrow_cl",
  "template_invoice_id": "ORIGINAL_INV",
  "new_invoice_id": "INV_2025_001",
  "admin": "<ADMIN_ADDRESS>",
  "sme_address": "<SME_ADDRESS>",
  "yield_bps": 800,
  "maturity": 1735689600,
  "new_amount": 750000000000
}
```

## Workflow Example

### Scenario: Monthly Invoice Cycles

**Month 1:**
1. Create escrow for invoice "INV_JAN_2025" (target: 1M USDC)
2. SME receives offers, investors fund it
3. After invoicing period, settle it

**Month 2:**
1. Clone from settled January escrow → new escrow for "INV_FEB_2025" (target: 1.2M USDC)
2. Same SME, same terms, new invoice
3. Repeat funding cycle

**Benefits:**
- No need to re-specify yield structure, maturity, or caps
- Single CLI call instead of full `init` call
- Audit trail shows template relationship
- Reduces configuration errors

## Error Handling

### Error 170: CloneNotSettled

**Cause**: Template escrow status ≠ 2

**Solution**:
1. Verify template escrow has been settled
2. Check: `get_escrow_summary()` → `status == 2`
3. If not settled, call `settle()` first (if eligible)
4. If not eligible for settlement, create new escrow with `init`

### Error 171: CloneAmountNotPositive

**Cause**: `new_amount <= 0`

**Solution**:
1. Ensure `new_amount` is positive
2. Check token decimals (e.g., USDC has 6 decimals)
3. For 1M USDC: pass `1_000_000_000_000` (12 zeros)

### Error 216: InvalidFundingTarget

**Cause**: Template escrow has `funding_target <= 0`

**Solution**:
1. Verify template escrow has a valid, strictly positive funding target
2. Ensure the template was initialized properly and has not suffered state corruption

### Error 3: EscrowAlreadyInitialized

**Cause**: The target contract has already been initialized

**Solution**:
1. Deploy a fresh, uninitialized contract instance before invoking `clone_settled_escrow`


### Error 4: InvoiceIdInvalidLength

**Cause**: `new_invoice_id` is empty or > 32 characters

**Solution**:
1. Keep invoice IDs short and descriptive
2. Valid: `"INV_2025_Q1"`, `"ACME_FEB_001"`, `"Test_123"`
3. Invalid: `"Invoice_For_February_2025_Company_ABC"` (too long)

### Error 5: InvoiceIdInvalidCharset

**Cause**: `new_invoice_id` contains invalid characters

**Solution**:
1. Use only: `A-Z`, `a-z`, `0-9`, `_` (underscore)
2. No spaces, dashes, symbols, or special characters
3. Valid: `"INV_001"`, `"acme_feb_2025"`, `"TEST123"`
4. Invalid: `"INV-001"`, `"INV 001"`, `"INV.001"`, `"INV@001"`

### Unauthorized Error

**Cause**: Caller is not the template admin

**Solution**:
1. Verify you're using the correct admin key
2. Check: `get_escrow_summary()` → `escrow.admin`
3. Must match your signing key

## Best Practices

1. **Always verify settled status first**
   ```bash
   stellar contract invoke ... -- get_escrow_summary | jq '.escrow.status'
   # Should output: 2
   ```

2. **Use consistent invoice ID naming**
   - Example pattern: `COMPANY_MONTH_YEAR`
   - Makes auditing and tracking easier
   - Helps prevent duplicate IDs

3. **Document the clone relationship**
   - Note in your system: "INV_2025_002 cloned from INV_2025_001"
   - Helps with audit trails
   - Useful for future parameter changes

4. **Batch clone operations**
   - If cloning multiple times in sequence, do it in a single transaction batch
   - Reduces network calls
   - Better for operational efficiency

5. **Verify yield tiers after clone** (if configured)
   ```bash
   stellar contract invoke ... -- get_escrow_summary
   # Check if yield_tiers copied correctly
   ```

## Monitoring & Observability

### Check Cloned Escrow State

```bash
# Get full summary after clone
stellar contract invoke \
  --network testnet \
  --source ADMIN_KEY \
  -- get_escrow_summary
```

Expected state for new clone:
```json
{
  "escrow": {
    "invoice_id": "INV_2025_001",
    "admin": "<SAME_AS_TEMPLATE>",
    "sme_address": "<SAME_AS_TEMPLATE>",
    "amount": 750000000000,
    "funding_target": 750000000000,
    "funded_amount": 0,         // Always 0 for new clone
    "yield_bps": 800,           // Copied from template
    "maturity": 1735689600,     // Copied from template
    "status": 0                 // Always 0 (open)
  },
  "unique_funder_count": 0,      // Always 0 for new clone
  "legal_hold": false,           // Always false for new clone
  "schema_version": 6
}
```

### Track Clone Lineage

Use `EscrowCloned` events to track:
- Which template was used (template_invoice_id)
- Who initiated the clone (admin)
- When it was created (block timestamp)
- What parameters were used (amount, yield_bps, maturity)

Example indexing:
```
Template: INV_2025_001 (settled Jan 15)
├─ Clone 1: INV_2025_002 (Feb 1) - 1.2M USDC
├─ Clone 2: INV_2025_003 (Mar 1) - 1.5M USDC
└─ Clone 3: INV_2025_004 (Apr 1) - 1.1M USDC
```

## Troubleshooting

### Clone fails with "unauthorized"

1. Verify signing key matches template admin
2. Check account sequence number
3. Ensure sufficient network fees

### Clone succeeds but new escrow is missing

1. Check transaction receipt for success status
2. Verify new contract ID in event
3. Query new contract with returned ID

### Invoice ID characters rejected

1. Copy-paste from this guide's valid examples
2. Avoid hyphens (-) and spaces
3. Use lowercase + underscores for consistency

### Amount looks wrong after clone

1. Verify token decimals (usually 6 for USD stablecoins)
2. Example: 1,000 USDC = 1_000_000_000 (9 digits)
3. Check escrow amount matches your intent

## Support & Escalation

If issues persist:
1. Check contract logs with `--verbose` flag
2. Review error code documentation: `docs/escrow-error-messages.md`
3. Verify template escrow status one more time
4. Consider creating new escrow with `init` if template is somehow invalid
