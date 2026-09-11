# Follow-up

Issues noticed during remediation that are outside the finding being fixed. Not fixed here.

- **Canonical frontend plan missing.** `autotherm-crm-frontend-plan.md` was not in the workspace;
  `FRONTEND_PLAN.md` was assembled from the audit spec and the remediation rule corrections
  (see its provenance note). Replace it with the canonical document and re-check the section
  numbers cited in source comments (`§5`, `§12`, `§13`, `§14`).
- **Backend test failing before remediation started.**
  `backend/tests/smtp.rs::unreachable_server_fails_the_connection_test_with_a_hint` fails in the
  baseline commit (`0f049ba`); the other six SMTP tests pass.
