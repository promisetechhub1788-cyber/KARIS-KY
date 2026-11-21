# RFC Process — Request for Comments

The RFC process is a lightweight mechanism for documenting design decisions, gathering feedback, and building institutional knowledge about the karis-ky escrow contract and broader platform architecture.

**RFCs differ from ADRs:**
- **ADRs** (Architecture Decision Records): *Accepted* decisions already implemented or planned for near-term release
- **RFCs**: *Proposed* features, changes, or explorations that require discussion before commitment

---

## When to Write an RFC

Use an RFC when:

- **Proposing a new feature** (e.g., multi-token support, yield reinvestment)
- **Exploring a major change** (e.g., storage layout evolution, new role type)
- **Defining integration patterns** (e.g., registry interface, indexer contract)
- **Documenting operational procedures** that affect the team (e.g., deployment gates, audit cadence)
- **Discussing breaking changes** before implementation
- **Clarifying cross-team consensus** on a design direction

Do **not** write an RFC for:
- **Bug fixes** (use GitHub issues)
- **Minor optimizations** (inline code discussion)
- **Documentation updates** (PR comments)
- **Operational one-time decisions** (use Slack/email thread, then archive summary)

---

## RFC Lifecycle

```
DRAFT → DISCUSSION → ACCEPTED → IMPLEMENTED → CLOSED
  ↓          ↓           ↓          ↓           ↓
Review   Feedback    Decision   Reference    Archive
period   gathering   made       in code      in docs
```

### Phases

1. **DRAFT** (Author)
   - Author writes RFC using the template
   - Opens PR or discussion thread
   - Shares with key stakeholders (typically 3–5 domain experts)

2. **DISCUSSION** (Team)
   - Minimum 3-day review period (1 week preferred for major changes)
   - Stakeholders comment with concerns, alternatives, use cases
   - Author updates RFC based on feedback
   - Target resolution: all **must-have** comments addressed

3. **ACCEPTED** (Decision Maker)
   - Owner/lead approves and merges
   - RFC status updated to `Accepted`
   - Issue/PR linked for tracking
   - Assigned to implementer

4. **IMPLEMENTED** (Implementer)
   - Feature coded and tested
   - References RFC in commit messages and code comments
   - Cross-links added (RFC ← → ADR, GitHub issues)
   - RFC marked `Implemented`

5. **CLOSED** (Archive)
   - After feature ships to production or design is superseded
   - RFC marked `Closed`
   - Successor RFC linked if applicable

---

## RFC Metadata

Every RFC has a header with:

```yaml
# RFC-NNN: Title

**Status:** DRAFT | DISCUSSION | ACCEPTED | IMPLEMENTED | CLOSED  
**Author:** Name (GitHub handle)  
**Date Proposed:** YYYY-MM-DD  
**Target Release:** vX.Y (if known)  
**Related:** ADR-NNN, Issue #NNN, RFC-NNN (if applicable)  
```

---

## RFC Template

See [`TEMPLATE.md`](TEMPLATE.md) for the standard structure. Key sections:

1. **Summary** — One paragraph explaining the RFC
2. **Motivation** — Why this is needed; problem statement
3. **Design** — Proposed solution with examples
4. **Alternatives** — At least 2 alternatives considered + tradeoffs
5. **Implementation** — Effort estimate, milestones, blockers
6. **Acceptance Criteria** — How we know it's done
7. **Rollout Plan** — Staged rollout, testing, monitoring
8. **References** — Links to related issues, ADRs, external docs

---

## Examples

### RFC-001: Multi-Token Support
**Status:** DRAFT (hypothetical)  
**Scope:** Allow escrows to settle in multiple stablecoin assets (USDC, EURC, etc.)  
**Complexity:** High (storage schema, settlement logic, token validation)

### RFC-002: Yield Reinvestment
**Status:** DISCUSSION (hypothetical)  
**Scope:** Auto-compound investor yield in next funding round  
**Complexity:** Medium (accounting, per-investor state)

### RFC-003: Registry Integration
**Status:** ACCEPTED (hypothetical)  
**Scope:** Link escrow instances to a registry for discoverability  
**Complexity:** Low (read-only reference, no contract migration)

---

## Process Rules

1. **Numbering:** Increment `RFC-NNN` sequentially; never reuse
2. **File naming:** `RFC-NNN-kebab-case-title.md`
3. **Minimum review:** 3 days for minor, 1 week for major changes
4. **Consensus bar:** No blocking objections from domain owners; not unanimous
5. **Async-friendly:** Use written RFC + PR comments, not synchronous meetings
6. **Link forward:** RFC must link to implementation (commit, PR, ADR)

---

## Linking RFCs, ADRs, and Issues

### From RFC to ADR (after acceptance + implementation)
```markdown
# RFC-001: Multi-Token Support

... [design details] ...

**Implemented as:** [ADR-009: Multi-Token Settlement Model](../adr/ADR-009-multi-token-settlement.md)
```

### From ADR to RFC
```markdown
# ADR-009: Multi-Token Settlement Model

**Based on RFC:** [RFC-001: Multi-Token Support](../rfc/RFC-001-multi-token-support.md)
```

### From GitHub Issue to RFC
```markdown
Fixes: [#1234 — Support EURC in addition to USDC](https://github.com/karis-ky/issues/1234)
RFC: [RFC-001: Multi-Token Support](../rfc/RFC-001-multi-token-support.md)
```

---

## Review Checklist

Reviewers should check:

- [ ] Problem is clearly motivated
- [ ] Proposed design solves the problem
- [ ] Tradeoffs are explicit
- [ ] Alternatives are considered
- [ ] Implementation effort is realistic
- [ ] Acceptance criteria are testable
- [ ] No unresolved blockers
- [ ] Links to related docs are present

---

## FAQ

### Q: Can I convert a GitHub issue to an RFC?
**A:** Yes. Create RFC using the issue description as seed. Link both.

### Q: How long should review take?
**A:** 3 days minimum; aim for 1 week for complex changes. Adjust per urgency.

### Q: What if we disagree on an RFC?
**A:** Document the disagreement in the RFC. Decision maker breaks tie and documents rationale in `Decision` section.

### Q: Can RFCs be rejected?
**A:** Yes. Mark as `CLOSED: Rejected` and explain why. Reference future RFC if direction changes.

### Q: How do I deprecate an RFC?
**A:** If superseded, mark original as `CLOSED: Superseded by RFC-NNN`. Link to successor.

### Q: Should all RFCs become ADRs?
**A:** No. Simple RFCs may not warrant formal ADR. Only upgrade if decision is architectural + long-lived.

---

## Index

| RFC | Title | Status | Target Release | Notes |
|-----|-------|--------|-----------------|-------|
| [RFC-001](RFC-001-multi-token-support.md) | Multi-Token Support | DRAFT | v2.0 | Extends escrow to support multiple settlement tokens |
| [RFC-002](RFC-002-yield-reinvestment.md) | Yield Reinvestment | DISCUSSION | v1.5 | Auto-compound investor yield across funding rounds |
| [RFC-003](RFC-003-registry-integration.md) | Registry Integration | ACCEPTED | v1.4 | Discoverability via centralized registry contract |
| [RFC-006](RFC-006-multi-token-escrow.md) | Multi-Token Escrow Support | DRAFT | v2.0 | Evaluates token basket, per-token shards, and registry abstraction |

---

## Getting Started

1. **Read** the [template](TEMPLATE.md)
2. **Check** the index above for existing RFCs
3. **Create** a new file: `RFC-NNN-kebab-case-title.md`
4. **Share** with stakeholders (PR or discussion thread)
5. **Iterate** based on feedback
6. **Merge** when approved

---

## Related Documentation

- **ADRs:** [`docs/adr/`](../adr/) — Accepted architectural decisions
- **Architecture:** [`docs/arch/`](../arch/) — Visual diagrams and design overview
- **Runbook:** [`docs/OPERATOR_RUNBOOK.md`](../OPERATOR_RUNBOOK.md) — Operational procedures
- **Contract README:** [`escrow/README.md`](../../escrow/README.md) — Development guide
