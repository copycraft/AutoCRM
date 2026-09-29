# History

Point-in-time documents from the September 2026 audit and remediation. They describe the
tree as it was when they were written, not as it is now. They are kept unedited because
source comments and `docs/logic-audit/` cite them by line number.

For how the system works today, read [`../../README.md`](../../README.md) and
[`../DECISIONS.md`](../DECISIONS.md). Where one of these files disagrees with them, it is
out of date.

| File | What it was | Superseded where |
|---|---|---|
| [FRONTEND_PLAN.md](FRONTEND_PLAN.md) | The frontend brief, reassembled from the audit spec because the canonical plan was missing. Source comments still cite its sections (`§5`, `§12`, `§13`, `§14`). | Its "invoicing, VAT and billing are out of scope" rules (§1 and N1 in §4): invoicing was built, see the README's Scope section. |
| [AUDIT.md](AUDIT.md) | Frontend audit against that brief, before remediation. | The BLOCKERs and MAJORs were closed; a few MINORs were deferred. See REMEDIATION.md. |
| [REMEDIATION.md](REMEDIATION.md) | Work-through of the audit findings, R0–R9. | Complete. |
| [FOLLOWUP.md](FOLLOWUP.md) | Issues noticed during remediation and left for later. | Partly picked up by `docs/integration-audit/` and `docs/logic-audit/`. |
| [VIABILITY.md](VIABILITY.md) | Can Autotherm run on this? Reviewed against migrations `0001`–`0008`. | Migrations `0009`–`0030` came later: tasks, intake slip, inspections, invoicing, newsletter. |
