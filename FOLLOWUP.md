# Follow-up

Issues noticed during remediation that are outside the finding being fixed. Not fixed here.

- **Canonical frontend plan missing.** `autotherm-crm-frontend-plan.md` was not in the workspace;
  `FRONTEND_PLAN.md` was assembled from the audit spec and the remediation rule corrections
  (see its provenance note). Replace it with the canonical document and re-check the section
  numbers cited in source comments (`§5`, `§12`, `§13`, `§14`).
- **Backend test failing before remediation started.**
  `backend/tests/smtp.rs::unreachable_server_fails_the_connection_test_with_a_hint` fails in the
  baseline commit (`0f049ba`); the other six SMTP tests pass.
- **Residual `as` casts on API-adjacent data (post-R1).** The 7 non-null assertions are gone,
  but ~28 `as` casts remain (e.g. `(order.currency === 'EUR' ? 'EUR' : 'HUF') as Currency`,
  `JSON.parse(text) as T`-style narrowing in client code, `as const` literals). None were
  introduced to silence the R1.6 flag errors (tsc is clean without them), but M2's "no `as`"
  bar is not fully met. Left for a dedicated typing pass, not any R2–R9 finding.
- **Next.js major migration (14.2.35 → 16.x).** 14.2.35 is the latest 14.x; stable is 16.3.5.
  Deliberately not done during remediation. Known blockers, each verified against the migration
  guides, not guessed: (1) `middleware.ts` must become `proxy.ts` (16 renames/deprecates
  middleware — our next-intl locale middleware included); (2) route `params`/`searchParams`
  become async — every `[id]` page, `new` page and layout touching them needs `await`
  (~10 files); (3) `next lint` is removed — the `lint` script and the CI frontend job need
  an ESLint 9 flat-config replacement; (4) React 18 → 19 for the whole tree (RHF, Radix,
  TanStack compat to re-verify with zero frontend tests as a net); (5) next-intl 3.26.5
  against Next 16 needs a compat check, possibly a major of its own. Do this as one
  dedicated task on a green tree with a full regression pass, not layered over other work.
