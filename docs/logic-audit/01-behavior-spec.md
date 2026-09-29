# Logic audit: behavior spec

What the app is supposed to do, per user journey, traced through web, Android, API, service and DB. Each statement cites its source and a confidence level:

- **documented**: explicitly written (docs, comments, API contract)
- **implied**: suggested by UI text, tests or constraints
- **assumed**: domain common sense; never used alone to justify a code change

Open questions and contradictions are in [00-questions.md](00-questions.md). Findings and fixes are in [02-logic-errors.md](02-logic-errors.md).

Note: Hungarian VAT-law citations in the invoicing section were written from memory and are marked *assumed* until checked against the statute.

## Contents

- [Auth & access](#auth)
- [Sales pipeline](#sales)
- [Order lifecycle](#orders)
- [Inspection & media](#inspection)
- [Invoicing & money](#invoicing)
- [Email & communications](#email)
- [Reports, dashboard, search & time](#reports)

<a id="auth"></a>

## Auth & access

Scope: signing in on web (httpOnly cookie) and Android (`client: "mobile"` → bearer token), session lifetime / idle expiry / revocation, logout, forced password change, password policy, failed-login lockout, the four-role capability matrix as enforced by the backend and mirrored by both clients, and staff user administration (create, edit role/name/active, reset password, revoke sessions). Entry points: web `/[locale]/login`, `/[locale]/password`, sidebar logout, `AppShell` gates; Android `LoginScreen`, `ChangePasswordScreen`, drawer "Kijelentkezés", `ServerSetupScreen`; API `POST /api/auth/login|logout|password`, `GET /api/auth/me|sessions`, `DELETE /api/auth/sessions/{id}`, `GET|POST /api/users`, `PATCH /api/users/{id}`, `POST /api/users/{id}/password|revoke-sessions`; CLI `create-admin`. Key files: `backend/src/service/auth.rs`, `backend/src/api/auth.rs`, `backend/src/api/extract.rs`, `backend/src/api/users.rs`, `backend/src/repo/users.rs`, `backend/src/repo/sessions.rs`, `backend/src/domain/role.rs`, `backend/src/error.rs`, `backend/migrations/0001_foundation.sql`, `frontend/src/lib/auth/context.tsx`, `frontend/src/components/layout/AppShell.tsx`, `frontend/src/app/[locale]/login/page.tsx`, `frontend/src/app/[locale]/password/page.tsx`, `android/.../data/auth/SessionStore.kt`, `android/.../ui/login/LoginScreen.kt`, `android/.../ui/login/ChangePasswordScreen.kt`, `android/.../MainActivity.kt`. (Android paths abbreviate `android/app/src/main/java/hu/autotherm/autocrm`.) No backend integration test covers auth (`backend/tests/*` only builds users via `common::user`, `backend/tests/common/mod.rs:95`).

### Login (backend)

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| AUTH-01 | `POST /api/auth/login` accepts `{email, password, client?, device_label?}`; when `client` is omitted it defaults to `web`. | backend/src/api/auth.rs:33-45; docs/API.md:40 | documented |
| AUTH-02 | Email lookup is case-insensitive and ignores surrounding whitespace (`lower(email) = lower(trim($1))`); emails are unique case-insensitively at DB level (`users_email_key ON lower(email)`). | backend/src/repo/users.rs:47-48; backend/migrations/0001_foundation.sql:29 | documented |
| AUTH-03 | When the email does not exist, the server still performs one Argon2 verification (against a dummy hash) and returns 401 `unauthenticated`, so response timing does not reveal which addresses have accounts. | backend/src/service/auth.rs:108-115,143-146; docs/DECISIONS.md:138 | documented |
| AUTH-04 | When the account's `locked_until` is in the future, login returns 429 `too_many_requests` ("too many attempts, try again later") before the password is checked. | backend/src/service/auth.rs:147-149; backend/src/error.rs:42-43,104-108; docs/API.md:22 | documented |
| AUTH-05 | A wrong password increments `failed_logins`; when the count reaches 10, `locked_until = now() + 15 minutes`. Response is 401 `unauthenticated`. | backend/src/repo/users.rs:7-9,141-154; backend/src/service/auth.rs:150-154; docs/DECISIONS.md:138 | documented |
| AUTH-06 | A correct password for a deactivated account (`is_active = false`) returns 401 `unauthenticated` (indistinguishable from a wrong password) and creates no session. | backend/src/service/auth.rs:155-157 | implied |
| AUTH-07 | A successful login resets `failed_logins` to 0 and clears `locked_until`. | backend/src/repo/users.rs:156-161; backend/src/service/auth.rs:158 | documented |
| AUTH-08 | Each successful login creates a new `sessions` row with a 256-bit random token (43-char base64url); only `sha256(token)` is stored (`token_hash BYTEA UNIQUE`), never the token. | backend/src/service/auth.rs:117-124,160-176; backend/migrations/0001_foundation.sql:37-39; docs/DECISIONS.md:135 | documented |
| AUTH-09 | The session records `kind` (`web`/`mobile`), `device_label` (trimmed, blank→null, truncated to 100 chars), `user_agent` (≤300 chars) and `ip` (first `X-Forwarded-For` entry, ≤64 chars). | backend/src/api/auth.rs:117-127; backend/src/api/extract.rs:93-106 | documented |
| AUTH-10 | Web login (`client: web`) returns `{user}` with no `token`/`expires_at` fields and sets cookie `autocrm_session`: `HttpOnly`, `SameSite=Lax`, `Path=/`, `Secure` per `COOKIE_SECURE`, `Max-Age` = seconds until the session's `expires_at`. | backend/src/api/auth.rs:132-152; docs/API.md:8; docs/DECISIONS.md:135 | documented |
| AUTH-11 | Mobile login (`client: mobile`) returns `{user, token, expires_at}` and sets no cookie; the client must send `Authorization: Bearer <token>`. | backend/src/api/auth.rs:86-95,153-160; docs/API.md:9-10 | documented |
| AUTH-12 | Login-CSRF: a `web` login that carries an `Origin` header must match `ALLOWED_ORIGINS` or `PUBLIC_BASE_URL` (trailing slash ignored), else 403 `forbidden`. A web login without `Origin` is not origin-checked; mobile logins are never origin-checked. | backend/src/api/auth.rs:113-116; backend/src/api/extract.rs:80-91 | documented |
| AUTH-13 | The login response `user` (`SessionUser`) has exactly `{id, email, display_name, role, must_change_password, session_kind}` and is the same shape as `GET /auth/me`'s `user`. | backend/src/api/auth.rs:47-58,97-100 | documented |
| AUTH-14 | Login succeeds (session created, 200) even when `must_change_password = true`; the flag is returned so the client can route to the change screen. | backend/src/service/auth.rs:142-186; backend/src/api/auth.rs:54-56 | implied |

### Authentication of requests, session lifetime, CSRF

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| AUTH-15 | A request authenticates by `Authorization: Bearer <token>` (prefix `Bearer ` or `bearer `) if present, otherwise by the `autocrm_session` cookie; with neither → 401 `unauthenticated`. Bearer takes precedence when both are present. | backend/src/api/extract.rs:21-42,69-75 | documented |
| AUTH-16 | Tokens that are empty or longer than 128 chars are rejected with 401 without a DB lookup. | backend/src/service/auth.rs:189-191 | documented |
| AUTH-17 | A session is valid only if `revoked_at IS NULL`, `expires_at > now()` and the user `is_active`; otherwise 401 `unauthenticated`. Deactivating a user therefore invalidates all their sessions immediately even without revocation. | backend/src/repo/sessions.rs:55-70 | documented |
| AUTH-18 | Web sessions: 7-day idle timeout, 30-day absolute lifetime. Mobile sessions: 60-day idle, 365-day absolute. `expires_at = min(now + idle, created_at + absolute)`. | backend/src/service/auth.rs:26-42,266-279; docs/DECISIONS.md:135-136 | documented |
| AUTH-19 | Sliding renewal: on an authenticated request, if more than 5 minutes have passed since `last_seen_at`, `last_seen_at` and `expires_at` are rewritten (never beyond the absolute cap). Requests within 5 minutes do not write. | backend/src/service/auth.rs:23-24,195-206 | documented |
| AUTH-20 | Sessions of users with `must_change_password = true` are NOT renewed by activity; they expire at their original `expires_at`. | backend/src/service/auth.rs:196-199 | documented |
| AUTH-21 | The role and `must_change_password` used for authorization are read live from `users` on every request (join), not cached in the session. | backend/src/repo/sessions.rs:59-65 | documented |
| AUTH-22 | CSRF: cookie-authenticated requests with a method other than GET/HEAD/OPTIONS must carry an allowed `Origin`; missing or foreign `Origin` → 403 `forbidden`. Bearer-authenticated requests are exempt. | backend/src/api/extract.rs:44-48,77-91; docs/DECISIONS.md:137; docs/API.md:8-9 | documented |
| AUTH-23 | In production (`APP_ENV=production`) the server refuses to start if `COOKIE_SECURE` is false or `PUBLIC_BASE_URL` is not `https://`. `COOKIE_SECURE` defaults to true. | backend/src/config.rs:345,467-479; README.md:104 | documented |
| AUTH-24 | Every error is `{"error":{"code","message"}}`; 401 `unauthenticated` "authentication required"; 403 `forbidden` "you do not have permission to do this"; 429 `too_many_requests`. | backend/src/error.rs:1-2,26-48,89-108; docs/API.md:12-22 | documented |

### Forced password change & password policy

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| AUTH-25 | New DB users default to `must_change_password = true`; admin-created users (`POST /users`) and admin password resets always set it to true. | backend/migrations/0001_foundation.sql:23; backend/src/api/users.rs:1-2,66,179; docs/DECISIONS.md:139 | documented |
| AUTH-26 | While `must_change_password` is true, every authenticated endpoint except `/api/auth/me`, `/api/auth/password`, `/api/auth/logout` returns 422 `password_change_required` ("you must change your password before continuing"). Unauthenticated endpoints are unaffected. | backend/src/api/extract.rs:17-19,52-64; backend/src/api/auth.rs:54-56; docs/API.md:21 | documented |
| AUTH-27 | The bootstrap admin created by the `create-admin` CLI has `must_change_password = false`; its password comes from `AUTOCRM_ADMIN_PASSWORD` or stdin and must satisfy the same policy. | backend/src/main.rs:33,211-232; README.md:37 | documented |
| AUTH-28 | Password policy for any new password (self-change, admin create, admin reset, CLI): at least 12 and at most 256 Unicode characters (counted as `chars`), no other composition rule. Violations → 400 `validation` "password must be at least 12 characters" / "password is too long". | backend/src/service/auth.rs:21-22,69-80,281-285 | documented |
| AUTH-29 | `POST /auth/password {current_password, new_password}`: new password validated first (400); then `current_password` must verify, else 422 `wrong_password` "current password is incorrect". | backend/src/service/auth.rs:218-234; backend/src/api/auth.rs:186-208 | documented |
| AUTH-30 | On successful self-change (204): hash replaced, `must_change_password = false`, `failed_logins = 0`, `locked_until = NULL`, and every other session of the user revoked, while the calling session stays valid — all in one transaction. | backend/src/service/auth.rs:235-240; backend/src/repo/users.rs:124-139; docs/API.md:43 | documented |
| AUTH-31 | Passwords are hashed with Argon2id (PHC string) and hashing/verification runs off the async executor. | backend/src/service/auth.rs:82-106,247-254; backend/migrations/0001_foundation.sql:21 | documented |
| AUTH-32 | There is no self-service "forgot password" path; recovery is admin reset (`POST /users/{id}/password`) or, for the last admin, the CLI/DB. | docs/history/VIABILITY.md:225; backend/src/api/users.rs:162-194 | documented |

### Logout & own sessions

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| AUTH-33 | `POST /auth/logout` revokes only the current session (`revoked_at = now()`), removes the `autocrm_session` cookie (Path=/), returns 204. Allowed while a password change is pending. | backend/src/api/auth.rs:164-176; backend/src/api/extract.rs:18-19; docs/API.md:41 | documented |
| AUTH-34 | `GET /auth/sessions` lists the caller's own non-revoked, non-expired sessions ordered by `last_seen_at DESC`, each with `current: true` on the calling one. | backend/src/api/auth.rs:210-235; backend/src/repo/sessions.rs:127-138; docs/API.md:44 | documented |
| AUTH-35 | `DELETE /auth/sessions/{id}` revokes only a session owned by the caller that is not already revoked (204); anything else (someone else's id, already revoked, nonexistent) → 404 `not_found` "session not found". | backend/src/api/auth.rs:237-252; backend/src/repo/sessions.rs:87-96 | documented |

### Roles & capability matrix (backend is the security boundary)

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| AUTH-36 | Exactly four roles exist: `admin`, `office`, `designer`, `viewer` (DB enum `user_role`, JSON lowercase). | backend/migrations/0001_foundation.sql:14; backend/src/domain/role.rs:6-16 | documented |
| AUTH-37 | Every authenticated user may read orders, partners, leads, images and reports; capabilities gate everything beyond reading. A missing capability → 403 `forbidden`. | backend/src/domain/role.rs:18-19; backend/src/service/auth.rs:55-62; docs/API.md:28 | documented |
| AUTH-38 | Admin only: `ManageUsers`, `ManageSettings`, `ManageConfiguration`, `OperateSystem`, `AnnulInvoices`. | backend/src/domain/role.rs:50-53; docs/API.md:30-32 | documented |
| AUTH-39 | Admin + office: `EditPartners`, `EditLeads`, `EditOrders`, `DeleteMedia`, `ViewOriginalImages`, `SendEmail`, `IssueInvoices`. | backend/src/domain/role.rs:54-57,37-39; docs/API.md:33 | documented |
| AUTH-40 | Admin + office + designer: `ChangeStages`, `ManageBlockers`, `UploadMedia`. Viewer has no capability at all. | backend/src/domain/role.rs:58-60,69-92; docs/API.md:34 | documented |
| AUTH-41 | Route gates: `/users*` → ManageUsers; `/admin/*` → OperateSystem; `PUT /settings` → ManageSettings; stage-definitions, project-types, email-templates (create/patch), `DELETE /email-suppressions/{email}`, `PUT /inspections/templates/{set}` → ManageConfiguration. | backend/src/api/users.rs:36,58,99,174,211; backend/src/api/admin.rs:55,96,139,159,186,246; backend/src/api/configuration.rs:113,176,291,352,560; backend/src/api/email.rs:240,291,370; backend/src/api/inspections.rs:854 | documented |
| AUTH-42 | Order stage changes (`POST /orders/{id}/stage`) need ChangeStages (designer allowed), but lead stage changes (`POST /leads/{id}/stage`) need EditLeads (designer gets 403). Lead conversion needs EditLeads AND EditOrders. | backend/src/api/orders.rs:756; backend/src/api/leads.rs:322,357-358; frontend/src/lib/auth/context.tsx:72-73 | documented |
| AUTH-43 | Inspections (create/patch/delete/photos/damages/signatures/sign/notes/verdicts) require ChangeStages. | backend/src/api/inspections.rs:152,349,403,442,509,551,580,614,662,758 | documented |
| AUTH-44 | `GET /images/{id}/original` requires ViewOriginalImages (a gated read); designers and viewers only see derived copies. | backend/src/api/media.rs:184; docs/DECISIONS.md:67 | documented |
| AUTH-45 | Invoices: issue, storno and proforma require IssueInvoices (admin, office); annul requires AnnulInvoices (admin only). | backend/src/api/invoices.rs:104,152,173,231; backend/src/domain/role.rs:37-42 | documented |
| AUTH-46 | Cancelling an email requires SendEmail and, additionally, being its sender (`sent_by = me`) unless the caller has OperateSystem; else 403. Retrying an email requires OperateSystem. | backend/src/api/email.rs:143-149,169; docs/API.md:136 | documented |
| AUTH-47 | Tasks (`POST /tasks`, `/tasks/{id}/done`, `DELETE /tasks/{id}`) have no capability gate: any authenticated user, including viewer, may create and toggle them; `created_by` records the author. | backend/src/api/tasks.rs:1-6 | documented (see Q-AUTH-4) |
| AUTH-48 | `POST /newsletter/subscribe` is not session-authenticated; it requires the `X-Newsletter-Key` website key (403 on missing/wrong key). | backend/src/api/newsletter.rs:109-124 | documented |

### User administration

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| AUTH-49 | There is no self-registration; only an admin creates accounts via `POST /users {email, display_name, role, temporary_password}` → 201 `User`. | backend/src/api/users.rs:1-2,40-78; docs/API.md:46 | documented |
| AUTH-50 | On create: email normalised and validated (400 "email is not a valid address"), `display_name` trimmed and required (400 "display_name is required"), temporary password meets the policy (AUTH-28); duplicate email (case-insensitive) → 409 `duplicate`. | backend/src/api/users.rs:59-63; backend/src/api/mod.rs:145-153; backend/src/error.rs:115-122; backend/migrations/0001_foundation.sql:29 | documented |
| AUTH-51 | Create, update and password reset each write an `audit_log` row (`entity='user'`, actions `create` {email, role} / `update` (field diff) / `reset_password` {}) attributed to the acting admin, in the same transaction. | backend/src/api/users.rs:65-76,136-153,178-192 | documented |
| AUTH-52 | `GET /users` returns all users ordered active first, then by `display_name`; fields `{id, email, display_name, role, is_active, must_change_password, created_at, updated_at}` (never the hash). | backend/src/repo/users.rs:11-21,67-75; backend/src/api/users.rs:31-38 | documented |
| AUTH-53 | `PATCH /users/{id} {display_name?, role?, is_active?}`: absent fields keep their value; `display_name`, if sent, must be non-blank. Email cannot be changed through the API. Unknown id → 404 "user not found". | backend/src/api/users.rs:80-130; backend/src/repo/users.rs:100-122; docs/API.md:47 | documented |
| AUTH-54 | Last-admin guard: if the target is an active admin and the patch would change its role to non-admin or set `is_active=false`, and there is ≤1 active admin, → 422 `last_admin` "cannot demote or deactivate the last active admin". This applies to admins editing themselves as well. | backend/src/api/users.rs:111-120; docs/DECISIONS.md:140; frontend/src/messages/hu.json:692 | documented |
| AUTH-55 | When an update deactivates the user or changes their role, all of the user's sessions are revoked (they must sign in again everywhere). | backend/src/api/users.rs:131-135; docs/history/VIABILITY.md:42 | documented |
| AUTH-56 | `POST /users/{id}/password {temporary_password}` (admin): sets the new hash, `must_change_password = true`, clears `failed_logins`/`locked_until` (i.e. unlocks), revokes all of the user's sessions; 204; unknown id → 404. | backend/src/api/users.rs:157-194; backend/src/repo/users.rs:124-139; docs/API.md:48 | documented |
| AUTH-57 | `POST /users/{id}/revoke-sessions` (admin) revokes every live session of that user and returns `{revoked: <count>}`. | backend/src/api/users.rs:196-214; docs/API.md:49 | documented |
| AUTH-58 | Users are never hard-deleted through the API; departure is `is_active=false`, and references such as `assigned_to` keep pointing at the row. | backend/src/api/users.rs:23-29; docs/history/VIABILITY.md:42 | implied |

### Web client

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| AUTH-59 | The web session token is never readable by JS: requests use `credentials: 'include'` and nothing auth-related is kept in `localStorage`. | frontend/src/lib/api/client.ts:3,36-42; docs/history/FRONTEND_PLAN.md:32 | documented |
| AUTH-60 | The login form requires a syntactically valid email (message `validation.email`) and a non-empty password, trims the email, and always sends `client: 'web'`. | frontend/src/app/[locale]/login/page.tsx:13-16,52; frontend/src/lib/auth/context.tsx:30 | documented |
| AUTH-61 | After login: if `user.must_change_password` → redirect to `/{locale}/password`, else `/{locale}`. | frontend/src/app/[locale]/login/page.tsx:53 | documented |
| AUTH-62 | A failed login shows an inline `role="alert"` message; wrong credentials are meant to read "Hibás e-mail vagy jelszó" and a lockout "Túl sok sikertelen próbálkozás. A fiók 15 percre zárolva." | frontend/src/app/[locale]/login/page.tsx:55,81-83; frontend/src/messages/hu.json:106,695 | implied (see Q-AUTH-1) |
| AUTH-63 | If the login page is reached with `email`/`password` in the query string (native GET submit), both are scrubbed from the URL and the `auth.nativeSubmit` notice is shown. | frontend/src/app/[locale]/login/page.tsx:30-47; frontend/src/messages/hu.json:107 | documented |
| AUTH-64 | Session state = `GET /auth/me` (`data.user`), not retried; authenticated iff it returned a user. Unauthenticated users on any `AppShell` page are redirected to `/{locale}/login`. | frontend/src/lib/auth/context.tsx:20-59; frontend/src/components/layout/AppShell.tsx:56-58; docs/history/FRONTEND_PLAN.md:193 | documented |
| AUTH-65 | While `must_change_password` is true, every `AppShell` page redirects to `/{locale}/password`. | frontend/src/components/layout/AppShell.tsx:60-65; docs/history/FRONTEND_PLAN.md:105 | documented |
| AUTH-66 | The password page rejects a new password shorter than 12 chars client-side (`validation.weakPassword`), then calls `POST /auth/password`; on success it invalidates the `me` query and navigates to `/{locale}`; errors show inline. | frontend/src/app/[locale]/password/page.tsx:27-43; frontend/src/messages/hu.json:815 | documented |
| AUTH-67 | Logout (sidebar) calls `POST /auth/logout` and clears the cached user even if the call fails, which triggers the redirect to login. | frontend/src/lib/auth/context.tsx:38-45; frontend/src/components/layout/Sidebar.tsx:89-92 | documented |
| AUTH-68 | Web capability helpers mirror role.rs and are UI-only: `canEdit*`/`canSendEmail` = admin|office; `canChangeStage`/`canManageBlockers`/`canUploadMedia` = admin|office|designer; `canAdmin` = admin. Admin and Settings nav entries only show for admins. | frontend/src/lib/auth/context.tsx:70-109; frontend/src/components/layout/Sidebar.tsx:24-35,59; docs/history/FRONTEND_PLAN.md:33 | documented |
| AUTH-69 | A 403 on a read renders the permission state ("Nincs jogosultsága megtekinteni ezt az adatot."); 401/403/404 queries are not retried. | frontend/src/components/ui/ErrorState.tsx:16-20; frontend/src/lib/query/provider.tsx:18-22; frontend/src/messages/hu.json:764; docs/history/FRONTEND_PLAN.md:193 | documented |
| AUTH-70 | Error codes map to the `errors` catalogue by camelCasing (`last_admin`→`lastAdmin`, `too_many_requests`→`tooManyRequests`); `validation` always shows the backend message; an unmapped code shows the backend message. | frontend/src/lib/api/errors.ts:37-40,78-91 | documented |

### Android client

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| AUTH-71 | Android login always sends `client: "mobile"` explicitly on the wire, plus `device_label = "<MANUFACTURER> <MODEL>"` so the session is identifiable for revocation. | android/.../data/api/AutoCrmApi.kt:154-168; android/.../ui/login/LoginScreen.kt:68-71; android/app/src/test/java/hu/autotherm/autocrm/LoginRequestTest.kt:23-34; git 935d114 | documented |
| AUTH-72 | The login button is enabled only when email and password are non-blank and no request is in flight; email is trimmed. | android/.../ui/login/LoginScreen.kt:63-65,176 | documented |
| AUTH-73 | A login response without `token` shows "a szerver nem adott munkamenet-jegyet" and saves nothing. | android/.../ui/login/LoginScreen.kt:72-76 | documented |
| AUTH-74 | On success the token, `expires_at`, `id`, `email`, `display_name`, `role`, `must_change_password` are saved in app-private DataStore (also when a password change is pending). | android/.../ui/login/LoginScreen.kt:77-86; android/.../data/auth/SessionStore.kt:16-29,77-87 | documented |
| AUTH-75 | App routing: no server configured → server setup; no saved account → login; `mustChangePassword` → ChangePasswordScreen gating the whole app; otherwise the app. | android/.../MainActivity.kt:130-158 | documented |
| AUTH-76 | Login errors: 401 → "Hibás e-mail vagy jelszó."; network → "Nincs kapcsolat a szerverrel."; any other failure → "Nem sikerült bejelentkezni." (never a crash). | android/.../ui/login/LoginScreen.kt:87-107 | documented |
| AUTH-77 | ChangePasswordScreen: new password < 12 chars → "Az új jelszó legalább 12 karakter legyen."; new == current → "Az új jelszó nem egyezhet a régivel."; 422 `wrong_password` → "Hibás a jelenlegi jelszó."; on success it re-reads `/auth/me` and re-saves the account so the gate lifts. | android/.../ui/login/ChangePasswordScreen.kt:61-86,104-113 | documented |
| AUTH-78 | ChangePasswordScreen offers "Kijelentkezés": best-effort server logout, then local session cleared → login screen. | android/.../ui/login/ChangePasswordScreen.kt:96-102,193-196 | documented |
| AUTH-79 | Drawer logout requires a second tap ("Biztos? Koppints újra"), is disarmed when the drawer closes, calls logout best-effort, and always clears the local session. | android/.../MainActivity.kt:216-220,257-272 | documented |
| AUTH-80 | Clearing the session never clears the photo upload queue; background upload/inspection workers do nothing without a token. | android/.../data/auth/SessionStore.kt:89-96; android/.../data/upload/UploadWorker.kt:37; android/.../data/inspection/InspectionSyncWorker.kt:32 | documented |
| AUTH-81 | Changing the server address clears the saved session (a token belongs to one server). | android/.../ui/server/ServerSetupScreen.kt:114-128 | documented |
| AUTH-82 | Android UI capabilities mirror role.rs (UI-only): `canEdit` = admin|office; `canChangeStage` and `canUploadMedia` = admin|office|designer; inspections/annotations gated on `canChangeStage`. | android/.../data/auth/SessionStore.kt:49-57; android/.../MainActivity.kt:214,543,579 | documented |
| AUTH-83 | HTTP mapping: 401 → Unauthenticated ("A munkamenet lejárt. Jelentkezz be újra."), 403 → Forbidden ("Ehhez nincs jogosultságod."), 400/409/422 → Rule(code, message), network and 5xx retryable, 4xx not. | android/.../data/api/AutoCrmApi.kt:133-146; android/.../data/api/ApiError.kt:13-39; android/.../ui/common/Errors.kt:25-32 | documented |

<a id="sales"></a>

## Sales pipeline

Scope: partners (customers/suppliers, `kind` business/person) and their contacts; vehicles as they relate to partners; leads (create, edit, stage, assignment); the V2.3 quotation fields on the lead and the quotation letter; documents filed on a lead (V2.4) and document validity (V2.5); lead → order conversion (V2.7, repeat conversion); order relations and the supplier `role` from migration 0011.
Entry points. Web: `/[locale]/partners/{business,consumers,new,[id]}`, `/[locale]/leads`, `/[locale]/leads/new` (`?clone=`), `/[locale]/leads/[id]` with `LeadStageDialog`, `LeadConvertDialog`, `QuotationDialog`. Android: `ui/partners/PartnerScreens.kt`, `PartnerEditScreen.kt`, `ui/leads/LeadScreens.kt`, `LeadEditScreen.kt`. API: `/partners*`, `/contacts/*`, `/leads*`, `/leads/{id}/{stage,transitions,convert,quotation,uploads,documents}`, `/documents*`, `/vehicles*`, `/orders/{id}/vehicles`.
Key files: `backend/src/api/{partners,leads,vehicles,media}.rs`, `backend/src/service/{leads,stages,orders,media,email}.rs`, `backend/src/repo/{partners,contacts,leads,vehicles,documents}.rs`, `backend/src/domain/{partner,stage,media}.rs`, migrations 0002, 0010–0014, 0020, `frontend/src/components/forms/{PartnerForm,PartnerPicker,LeadForm,LeadConvertDialog}.tsx`, `docs/API.md`, `docs/DECISIONS.md`, `docs/history/VIABILITY.md`.

### Partners: fields, validation, normalisation

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| SALES-01 | Creating a partner requires `kind` ∈ {`business`,`person`} and a non-blank `name`. A blank name returns 400 `validation`. | backend/migrations/0002_partners_leads.sql:3,7; backend/src/api/partners.rs:129; frontend/src/components/forms/PartnerForm.tsx:14-15 | documented |
| SALES-02 | When `country` is omitted on create, it becomes `HU`. When `default_currency` is omitted, it becomes `HUF`. Only `HUF` and `EUR` are accepted as currency. | backend/src/api/partners.rs:192-193; backend/migrations/0002_partners_leads.sql:11-12; docs/API.md:56 | documented |
| SALES-03 | `country` is trimmed and upper-cased and must be exactly 2 ASCII letters (ISO 3166-1 alpha-2). `"at"` is stored as `AT`. `"DEU"` returns 400 "country must be a two-letter ISO code". | backend/src/domain/partner.rs:33-36,79-82; backend/src/api/partners.rs:112-113 | documented |
| SALES-04 | When `country = HU` and `tax_number` is not blank, the tax number must be 11 digits, with only digits, `-` and spaces allowed. It is stored in the canonical form `12345678-1-23`. `"12345678123"` and `" 12345678 1 23 "` both become `12345678-1-23`. `"1234"` and `"ATU12345678"` return 400 "Hungarian tax number must be 11 digits (12345678-1-23)". | backend/src/domain/partner.rs:15-31,60-76; backend/src/api/partners.rs:114-119; docs/API.md:56; backend/migrations/0002_partners_leads.sql:9 | documented |
| SALES-05 | When `country ≠ HU`, `tax_number` is stored as entered (trimmed) without the Hungarian format check. | backend/src/api/partners.rs:114-119; docs/history/VIABILITY.md:32 | documented |
| SALES-06 | The Hungarian tax-number format is `xxxxxxxx-y-zz`: an 8-digit core, a 1-digit VAT code and a 2-digit county code. The VAT code (y) can only be 1–5, and the 8th digit of the core is a check digit (weights 9,7,3,1,9,7,3). A number that fails these rules is not a valid adószám even when it has 11 digits. | Hungarian Act CL of 2017 (Art.) / NAV adószám structure; backend/src/domain/partner.rs:15 (only the digit count is documented) | assumed |
| SALES-07 | When `eu_tax_number` is set, spaces are removed and it is upper-cased (`"atu 123"` → `ATU123`). No further format check applies. | backend/src/api/partners.rs:131; backend/migrations/0002_partners_leads.sql:10 | documented |
| SALES-08 | When a partner or contact `email` is not blank, it must be a valid address and is stored normalised. Otherwise the request returns 400 "email is not a valid address". | backend/src/api/partners.rs:120-126,456-463 | documented |
| SALES-09 | In every optional text field, blank input means null. On PATCH, an omitted field keeps its value and `null` clears it. | docs/API.md:27; backend/src/api/partners.rs:437; frontend/src/components/forms/PartnerForm.tsx:49-72 | documented |
| SALES-10 | Creating, editing, archiving or unarchiving a partner, and creating, editing or archiving a contact, requires `EditPartners` (admin, office). Otherwise the request returns 403 `forbidden`. Any signed-in user can read partners. | backend/src/api/partners.rs:186,296,370,485,517,548; docs/API.md:29-33 | documented |
| SALES-11 | Each partner create, update and archive/unarchive writes an `audit_log` row (entity `partner`) in the same transaction. An update records a field-by-field diff of every changed column. | backend/src/api/partners.rs:205-213,324-359,375-383 | documented |
| SALES-12 | Partner search `q` matches name, tax number, EU tax number, e-mail and city, case-insensitively. It also matches phone in any `+36`/`06`/`0036` spacing variant: `"+36 30 123 4567"`, `"06-30-123-4567"` and `"0036 30 123 4567"` are the same number. | backend/src/api/partners.rs:38-40; backend/src/domain/partner.rs:38-54,84-96; backend/src/repo/partners.rs:90-94 | documented |
| SALES-13 | Archiving sets `archived_at` once: archiving again keeps the original timestamp. Unarchiving clears it. An unknown id returns 404. An archived partner is left out of `GET /partners` unless `include_archived=true`, and must not appear in new selections. | backend/src/repo/partners.rs:100,182-191; backend/src/api/partners.rs:364-374; frontend/src/messages/hu.json:152 | documented |
| SALES-14 | An order (including one created by lead conversion) cannot be created for an archived partner: 400 "partner is archived". | backend/src/service/orders.rs:52-58 | documented |
| SALES-15 | `GET /partners/{id}` returns the partner, all its contacts (archived included, active first, then by name), up to 100 of its orders, and every lead of the partner newest first, quoted or not. | backend/src/api/partners.rs:219-253; backend/src/repo/contacts.rs:39-54; backend/src/repo/leads.rs:224-244 | documented |

### Supplier / customer role (migration 0011, V2.6)

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| SALES-16 | `partners.role` is one of NULL, `customer`, `supplier`, `both`. Any other value returns 400 "role must be customer, supplier or both", on write and on the search filter. | backend/migrations/0011_relations_quotes_suppliers.sql:30-31; backend/src/api/partners.rs:71-77,141-149 | documented |
| SALES-17 | NULL role means "not yet classified" and counts as a customer. `?role=customer` returns partners with role `customer`, `both` or NULL. `?role=supplier` returns only `supplier` and `both`. | backend/migrations/0011_relations_quotes_suppliers.sql:32-33; backend/src/repo/partners.rs:24-27,96-99; backend/src/api/partners.rs:42-44 | documented |
| SALES-18 | A role is "set deliberately": no client may assign a role the user did not choose. | backend/migrations/0011_relations_quotes_suppliers.sql:33 | documented |
| SALES-19 | Suppliers (role `supplier`) should not appear in customer pickers (lead/order partner selection). That is the problem the flag exists to solve. | backend/migrations/0011_relations_quotes_suppliers.sql:27-29; docs/history/VIABILITY.md:40,133 | implied |
| SALES-20 | A role change is audited like any other partner field change. | backend/src/api/partners.rs:324-359 (diff lists every other column); docs/history/VIABILITY.md:30 (audit trail purpose) | implied |

### Contacts

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| SALES-21 | A contact always belongs to exactly one existing partner (`partner_id NOT NULL`). Creating one under an unknown partner returns 404 `not_found` ("partner"). | backend/migrations/0002_partners_leads.sql:31; backend/src/api/partners.rs:488-490 | documented |
| SALES-22 | Contact create requires a non-blank `name`. On PATCH, an omitted field is kept and `null` clears it. `name` cannot be cleared to blank. | backend/src/api/partners.rs:437,451-471 | documented |
| SALES-23 | An archived contact stays in history but must not appear in new selections. The web lead form lists only non-archived contacts of the chosen partner, and the contact select is disabled until a partner is chosen. | frontend/src/messages/hu.json:153; frontend/src/components/forms/LeadForm.tsx:214-217 | documented |
| SALES-24 | Contact create, update and archive are audited with entity `contact`. | backend/src/api/partners.rs:492-500,526-533,553-561 | documented |

### Vehicles in relation to partners (0010, V2.1)

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| SALES-25 | A vehicle needs a plate or a VIN. Neither returns 400 "a vehicle needs a plate or a VIN: a make and model do not identify a van". The DB enforces the same rule. | backend/src/api/vehicles.rs:84-88; backend/migrations/0010_vehicles.sql:29 | documented |
| SALES-26 | Plates are one vehicle regardless of case, spaces or dashes (`abc-123` = `ABC 123` = `ABC123`). VIN and plate are stored upper-cased. | backend/migrations/0010_vehicles.sql:15-19; backend/src/api/vehicles.rs:76-77 | documented |
| SALES-27 | `POST /vehicles` with a plate or VIN that already exists reuses the stored vehicle and returns `existing: true`, shown as "this van has been here before". A plate match wins over a VIN match. Existing values are never overwritten: the call only fills empty columns. | backend/src/api/vehicles.rs:97-100,116-124; backend/src/repo/vehicles.rs:52-76,120-122 | documented |
| SALES-28 | `vehicles.partner_id` is the usual owner and is optional. The order's partner stays the customer of record for that job, even if the van has since changed hands. | backend/migrations/0010_vehicles.sql:23-25 | documented |
| SALES-29 | When an order (including a converted lead) carries plate/VIN fields, a vehicle is created or matched and attached to the order. If the vehicle had no owner, the order's partner becomes its owner; an existing owner is kept. | backend/src/service/orders.rs:26-48; backend/src/repo/vehicles.rs:120-133 | documented |
| SALES-30 | Vehicle create, update, attach and detach require `EditOrders`. `year` must be between 1900 and 2200. | backend/src/api/vehicles.rs:113,181,226,247; backend/migrations/0010_vehicles.sql:22 | documented |

### Leads: create, edit, assignment

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| SALES-31 | Creating a lead requires only a non-blank `title`. Otherwise it returns 400 `validation`. The web form shows "required" under Cím. | backend/src/api/leads.rs:83,130; frontend/src/components/forms/LeadForm.tsx:22,203-204 | documented |
| SALES-32 | A lead can exist without a partner ("null while the enquirer is not yet a partner"), with free-text `contact_name`, `contact_email` and `contact_phone`. | backend/migrations/0002_partners_leads.sql:88-92 | documented |
| SALES-33 | If `partner_id` is given, the partner must exist (400 "partner does not exist"). If `contact_id` is given, the contact must exist (400 "contact does not exist") and belong to the lead's partner (400 "contact belongs to a different partner"). A contact without a partner is refused too. | backend/src/service/leads.rs:17-31 | documented |
| SALES-34 | A non-blank `contact_email` must be a valid address (400 "contact_email is not a valid address"). | backend/src/api/leads.rs:121-127 | documented |
| SALES-35 | A new lead starts in the lowest-positioned active, non-terminal, non-exit lead stage (seeded: `new`). The first `lead_stages` row is written in the same transaction as the lead, with `entered_by` = the creator. `created_by` is the creator. If no initial stage is configured, the request returns a 500 internal error. | backend/src/service/leads.rs:32-36; backend/src/domain/stage.rs:93-99; backend/migrations/0002_partners_leads.sql:70-75 | documented |
| SALES-36 | Creating or editing a lead requires `EditLeads` (admin, office). Creation writes an audit row `lead/create`. Every PATCH writes an audit diff covering all fields, including `assigned_to`, `quoted_value_minor`, `currency` and `quote_valid_until`. | backend/src/api/leads.rs:170,236,245-297; backend/src/service/leads.rs:37-45 | documented |
| SALES-37 | PATCH /leads/{id} runs under a row lock. An omitted field is kept, `null` clears it, and a blank title is refused. | backend/src/api/leads.rs:83,112-158,237-241; docs/API.md:27 | documented |
| SALES-38 | `assigned_to` is an optional user id. The lead list filters by `assigned_to` and shows the assignee's display name. The web form defaults to the last-used assignee on create. | backend/src/api/leads.rs:44,102; backend/src/repo/leads.rs:185,202; frontend/src/components/forms/LeadForm.tsx:144 | documented |
| SALES-39 | Lead search `q` matches title, contact name, contact e-mail and partner name, plus contact phone with Hungarian prefixes unified. `open=true` hides leads in terminal stages (`won`, `lost`). The default sort is newest first. | backend/src/repo/leads.rs:165-167,197-203; backend/src/api/leads.rs:45-47 | documented |
| SALES-40 | "Clone" on the web opens `/leads/new?clone={id}` pre-filled from the source lead and creates a new, independent lead. | frontend/src/app/[locale]/leads/new/page.tsx:26-55,70-84; frontend/src/app/[locale]/leads/[id]/page.tsx:142-146 | implied |

### Lead stages

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| SALES-41 | Seeded lead stages: `new`(10, stalls after 3 days), `contacted`(20, 7), `quoted`(30, 21), `won`(40, terminal), `lost`(50, terminal + exit). Stage keys never change; labels may. | backend/migrations/0002_partners_leads.sql:69-75 | documented |
| SALES-42 | Forward moves need no note. Backward moves need a non-blank note: without one, 422 `note_required`. Moving to `lost` (exit) is allowed from any open stage. Leaving a terminal stage (reopen) requires a note. Moving to the current stage, an unknown stage or an inactive stage is refused. | backend/src/domain/stage.rs:6-11,108-138; docs/DECISIONS.md:55-60; backend/src/service/stages.rs:276-291 | documented |
| SALES-43 | `POST /leads/{id}/stage` with `stage = won` always returns 422 `use_conversion` ("a lead is won by converting it to an order"). `GET /leads/{id}/transitions` lists `won` with `manual: false`. | backend/src/service/stages.rs:34,52-55,181-186,263-274; backend/src/domain/stage.rs:37-38; docs/DECISIONS.md:61; frontend/src/messages/hu.json:193 | documented |
| SALES-44 | Once a lead has at least one order, any stage change returns 422 `lead_converted` with a message naming the order numbers ("work on the order instead"). | backend/src/service/stages.rs:191-201; docs/DECISIONS.md:61 | documented |
| SALES-45 | Each lead stage change inserts a new `lead_stages` history row (no status column) and an audit row `stage_change` with from, to, kind and note. The current stage is the latest row. Changing a stage requires `EditLeads`. | backend/migrations/0002_partners_leads.sql:105-117; backend/src/service/stages.rs:214-224; backend/src/api/leads.rs:322 | documented |

### Quotation on the lead (0011 V2.3) and the quotation letter

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| SALES-46 | The quotation is not a separate `quotes` table and has no line items. It is `quoted_value_minor` (integer minor units: fillér/eurocent), `currency` (`HUF`/`EUR`) and `quote_valid_until` (date) on the lead. Revisions are traced through `audit_log` (entity `lead`). | backend/migrations/0011_relations_quotes_suppliers.sql:15-24; backend/src/repo/leads.rs:17-19; backend/src/api/leads.rs:283-295 | documented |
| SALES-47 | A quoted value without a currency is refused by the DB (`leads_quote_has_currency`: "A number with no currency is not a price"). | backend/migrations/0011_relations_quotes_suppliers.sql:23-24 | documented |
| SALES-48 | The web form takes the value in major units and converts it exactly (string/integer arithmetic, no floats). Unparseable input is not treated as blank: the form shows the "numeric" error and sends nothing, instead of silently clearing the quote. | frontend/src/components/forms/LeadForm.tsx:31-32,43-49,171-175; docs/history/FRONTEND_PLAN.md:65 | documented |
| SALES-49 | A quote is shown as expired ("Lejárt") when `quote_valid_until` is before today's date in Europe/Budapest. The web compares Budapest calendar dates on purpose, so the badge does not flip at UTC midnight. | frontend/src/app/[locale]/leads/[id]/page.tsx:31-36,242-244 | documented |
| SALES-50 | `POST /leads/{id}/quotation` requires `SendEmail` and returns 202 `{email_id}`. The recipient is the lead's `contact_email`, falling back to the partner's `email`. With neither, it returns 400 "lead #N has no email address: add a contact email first". | backend/src/api/leads.rs:389-406; backend/src/service/email.rs:648-665 | documented |
| SALES-51 | The default subject is "Árajánlatunk: {{lead.title}}". The default body adds "Ajánlott ár" and "Az ajánlat érvényes" lines only when the lead has those values. A lead without a price still sends (the PDF is authoritative). A blank hero returns 400 "hero is required". | backend/src/service/email.rs:621-624,675-691,712-718 | documented |
| SALES-52 | Unresolved `{{variables}}` refuse the send (400 listing them). `body_markdown=true` combined with `{{` returns 400. | backend/src/service/email.rs:692-710 | documented |
| SALES-53 | Quotation attachments must all be live documents of this lead. Any foreign or unknown id returns 400 "attachments must be documents of this lead". | backend/src/service/email.rs:726-735 | documented |

### Documents on leads (0012 V2.4, 0013/0014 V2.5, 0020)

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| SALES-54 | Each document belongs to exactly one owner: an order or a lead (`documents_one_owner`, `num_nonnulls(order_id, lead_id) = 1`). | backend/migrations/0012_documents_on_leads.sql:14-15; backend/src/repo/documents.rs:10-11,35 | documented |
| SALES-55 | `POST /leads/{id}/uploads` (requires `UploadMedia`) accepts only document targets. An image target on a lead returns 400 "images belong to an order, not a lead". An unknown lead returns 404. | backend/src/api/media.rs:63-80; backend/src/service/media.rs:104-108,124-130 | documented |
| SALES-56 | A lead document needs a filename with a permitted extension and a size ≤ 2 GiB (`MAX_DOCUMENT_BYTES`). The same content (sha256) uploaded twice to the same lead returns `already_uploaded` with the existing document id instead of a duplicate. | backend/src/service/media.rs:150-164; backend/src/domain/media.rs:61; backend/migrations/0012_documents_on_leads.sql:24-25 | documented |
| SALES-57 | Document kinds: `design`, `cad`, `certificate`, `invoice`, `proforma`, `other`. `invoice` documents are generated from NAV data and never uploaded. A `proforma` must never be mistaken for a tax document. | backend/src/domain/media.rs:44-58; backend/migrations/0020_document_kind_invoice.sql:1-4 | documented |
| SALES-58 | `valid_from` must not be after `valid_until`: the API returns 400 "valid_from is after valid_until" and the DB check `documents_validity_ordered` enforces the same. `GET /documents?kind=certificate&expiring_before=D` returns certificates whose validity ends on or before D. | backend/src/api/media.rs:286-294,346-350; backend/migrations/0014_document_validity.sql:11-15 | documented |
| SALES-59 | `GET /leads/{id}` and `GET /leads/{id}/documents` list the lead's non-deleted documents. The web lead page shows them with a count. | backend/src/api/leads.rs:190-191,214; backend/src/repo/documents.rs:122-130; frontend/src/app/[locale]/leads/[id]/page.tsx:254-273 | documented |
| SALES-60 | Deleting a lead document is a soft delete, requires `DeleteMedia`, and is audited against entity `lead`. | backend/src/api/media.rs:400-430 | documented |

### Lead → order conversion (V2.7)

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| SALES-61 | `POST /leads/{id}/convert` requires both `EditLeads` and `EditOrders` and returns 201 with the new order. An unknown lead returns 404. | backend/src/api/leads.rs:345-381 | documented |
| SALES-62 | Partner resolution: the body's `partner_id`, else the lead's. With neither, it returns 400 "choose or create a partner before converting the lead". The web shows "A lead-hez nincs partner rendelve…" and disables Convert. | backend/src/service/leads.rs:73-75; frontend/src/components/forms/LeadConvertDialog.tsx:76,102-104; frontend/src/messages/hu.json:197 | documented |
| SALES-63 | Title defaults to the lead's title when omitted or blank. A blank result returns 400. | backend/src/api/leads.rs:348,364-366; backend/src/api/orders.rs:303,306; docs/API.md:73 | documented |
| SALES-64 | When the body gives no `contact_id` and the order's partner is the lead's partner, the lead's `contact_id` is copied to the order. | backend/src/service/leads.rs:77-79 | documented |
| SALES-65 | The order goes through the same validation as `POST /orders`. The partner must exist and not be archived, the contact must belong to that partner, the currency must be HUF/EUR, and line items must share the currency. The order gets the next `YYYY-NNNN` number and the initial order stage (`intake`), with `orders.lead_id` = the lead. | backend/src/service/leads.rs:81-89; backend/src/service/orders.rs:52-68,91-113; docs/DECISIONS.md:22-28 | documented |
| SALES-66 | In the same transaction: if the lead had no partner, the chosen partner is written onto the lead. A `lead_stages` row `won` is inserted with note "Megrendelés: {order number}". An audit row `lead/convert` stores `order_id` and `order_number`. Everything commits or nothing does. | backend/src/service/leads.rs:91-112 | documented |
| SALES-67 | Conversion is repeatable: one enquiry for three identical vans becomes three orders, and all of them point back to the lead (the old unique constraint `orders_lead_id_key` was dropped). `GET /leads/{id}` returns every order with id and number. | backend/migrations/0011_relations_quotes_suppliers.sql:35-39; backend/src/service/leads.rs:69-71; backend/src/api/leads.rs:187-189,209-213 | documented |
| SALES-68 | Lead list and partner-detail lead rows show the first (lowest id) order of a converted lead, without duplicating the lead row. | backend/src/repo/leads.rs:194-196,242 | documented |
| SALES-69 | The web convert dialog pre-fills the currency from the partner's `default_currency` and lets the user change it (an EUR job for a HUF-default partner must be possible). The description is pre-filled from the lead. The order is created with no items. On success the browser goes to `/orders/{id}`. | frontend/src/components/forms/LeadConvertDialog.tsx:35-39,52-72; docs/history/REMEDIATION.md:13; docs/history/AUDIT.md:140-156 | documented |
| SALES-70 | Converting requires `EditLeads`. On the web, the Stage-change and Convert buttons are shown only to users who can edit leads, and only while the lead has no orders. | frontend/src/app/[locale]/leads/[id]/page.tsx:52,135,147-156 | implied |

### Order relations (0011 V2.2)

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| SALES-71 | `relation` ∈ {`warranty`,`rework`,`repeat`}. `related_order_id` and `relation` are set together or not at all. The related order must exist, and an order cannot relate to itself. Violations return 400 with the matching message. | backend/migrations/0011_relations_quotes_suppliers.sql:7-12; backend/src/service/orders.rs:69-86; backend/src/api/orders.rs:154-157 | documented |

<a id="orders"></a>

## Order lifecycle

Scope: creating and editing orders (required fields, numbering, line items, build spec, intake slip and its extras), the order stage machine (transitions, image gates, the intake-slip gate, notes, reopen, exit), who may do each step, automatic customer mail on stage change (the progress mail and the ready-for-pickup template from migration 0016), stalled-order alerts, blockers (create, update, resolve, reopen, nudges), follow-up tasks, the order audit trail, and the Android order picker and sticky capture order.

Entry points:
- **Web:** `/[locale]/orders/new` (`OrderForm`), `/[locale]/orders/[id]` (tabs data/items/invoices/stages/blockers/audit, `OrderStageDialog`, `StageRail`, `IntakeSlipSection`, `TaskList`), `/[locale]/board` (`PickupBoard`), `/[locale]/blockers`.
- **Android:** `OrderDetailScreen` (`StageDialog`, blocker and task dialogs), `TasksScreen`, `OrderPickerScreen` leading to `CaptureScreen`.
- **API:** `POST/GET /orders`, `GET/PATCH /orders/{id}`, `POST /orders/{id}/stage`, `GET /orders/{id}/transitions|stages|audit|notes|items`, `GET/PUT /orders/{id}/spec`, `PATCH/DELETE /order-items/{id}`, `GET/POST /orders/{id}/blockers`, `GET /blockers`, `PATCH /blockers/{id}`, `POST /blockers/{id}/resolve|reopen`, `/tasks*`, `GET /mobile/orders`.

Key files: backend/src/domain/stage.rs, backend/src/service/stages.rs, backend/src/repo/stages.rs, backend/src/service/orders.rs, backend/src/api/orders.rs, backend/src/domain/order.rs, backend/src/repo/orders.rs, backend/src/service/automation.rs, backend/src/api/blockers.rs, backend/src/repo/blockers.rs, backend/src/domain/blocker.rs, backend/src/api/tasks.rs, backend/src/repo/tasks.rs, backend/src/domain/role.rs, backend/src/error.rs, backend/migrations/0002, 0003, 0011, 0015–0019, frontend/src/components/forms/OrderStageDialog.tsx, frontend/src/app/[locale]/orders/[id]/page.tsx, frontend/src/components/board/PickupBoard.tsx, android/.../ui/common/StageDialog.kt, android/.../ui/orders/OrderDetailScreen.kt, android/.../ui/picker/OrderPickerScreen.kt, android/.../data/prefs/CapturePrefs.kt.

Citation conventions:
- "android/..." abbreviates `android/app/src/main/java/hu/autotherm/autocrm/`.
- "hu.json" is `frontend/src/messages/hu.json`.
- Status codes come from `backend/src/error.rs:89-150`: Validation is 400, Rule is 422, Conflict is 409, NotFound is 404, Forbidden is 403.

### Roles and permissions

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| ORD-01 | Every authenticated user, including `viewer`, may read orders, stage history, transitions, audit trail, notes, items, spec and blockers. Every write needs a capability. | backend/src/domain/role.rs:18-19; backend/src/api/orders.rs:96,457,771,786,877 (`Auth(_)` only) | documented |
| ORD-02 | Creating or editing an order, its items or its spec requires `EditOrders`, which only `admin` and `office` hold. A `designer` or `viewer` gets 403 `forbidden`. | backend/src/api/orders.rs:355,584,842,926,989,1063; backend/src/domain/role.rs:54-57,98; docs/API.md:33 | documented |
| ORD-03 | Changing an order's stage requires `ChangeStages`, which `admin`, `office` and `designer` hold. A `viewer` gets 403. | backend/src/api/orders.rs:756; backend/src/domain/role.rs:58-60,97; docs/API.md:34 | documented |
| ORD-04 | The web shows the "Fázisváltás" button only when `canChangeStage(user)` is true (admin/office/designer). The Edit button and the editable intake slip are shown only when `canEditOrders` is true (admin/office). | frontend/src/app/[locale]/orders/[id]/page.tsx:117-118,261,266,481,668; frontend/src/lib/auth/context.tsx:86-92 | implied |
| ORD-05 | Android shows the stage-change action and the blocker actions (new, resolve, reopen) only when `canChangeStage` is true (admin/office/designer). Item editing needs `canEdit` (admin/office). | android/.../data/auth/SessionStore.kt:54-56; android/.../ui/orders/OrderDetailScreen.kt:351,359,413,505-511,547-553,586 | implied |

### Order creation

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| ORD-06 | `POST /orders` without `partner_id` is rejected with 400 `validation` "partner_id is required". | backend/src/api/orders.rs:356-358; docs/API.md:80 | documented |
| ORD-07 | The title is trimmed and must be non-empty, otherwise 400 "title is required". On creation from a lead conversion, the lead's title is the fallback. | backend/src/api/orders.rs:303,306; backend/src/api/mod.rs:146-153; docs/API.md:73 | documented |
| ORD-08 | `currency` is required and must be `HUF` or `EUR`. The database enforces the same set. | backend/src/api/orders.rs:144; backend/migrations/0003_orders.sql:28 | documented |
| ORD-09 | When `valuation_date` is omitted, it defaults to the business "today" at creation. It must never mean "today at report time". | backend/src/api/orders.rs:145-146,311; docs/DECISIONS.md:25-28; backend/migrations/0003_orders.sql:29-31 | documented |
| ORD-10 | The partner must exist and must not be archived. A `contact_id` must belong to that partner. Violations return 400 "partner does not exist", "partner is archived" or "contact belongs to a different partner". | backend/src/service/orders.rs:50-68 | documented |
| ORD-11 | `related_order_id` and `relation` are set together or not at all. The relation must be `warranty`, `rework` or `repeat`. The related order must exist and must not be the order itself. These are 400s from the service; the DB CHECKs are the backstop. | backend/src/service/orders.rs:69-87; backend/migrations/0011_relations_quotes_suppliers.sql:9-12 | documented |
| ORD-12 | New orders are numbered `YYYY-NNNN`, per calendar year of the business day, with the highest existing `YYYY-` sequence plus 1. Allocation runs under a transaction-scoped advisory lock, so concurrent creates never collide. Imported numbers in other shapes are ignored by the sequence. Numbers are UNIQUE. | backend/src/domain/order.rs:7-9; backend/src/repo/orders.rs:151-162; docs/DECISIONS.md:30-31; backend/migrations/0003_orders.sql:22; backend/tests/orders.rs:103-126 | documented |
| ORD-13 | Creating an order inserts, in one transaction: the order, its items (positions 10, 20, 30…), the vehicle link, one `order_stages` row for the initial stage (the lowest-position active, non-exit, non-terminal stage, `intake` by seed), and an audit row `create` with `{number, lead_id, stage, items}`. If no initial stage is configured, the request fails with 500 and nothing is written. | backend/src/service/orders.rs:91-149; backend/src/domain/stage.rs:93-99,212-215 | documented |
| ORD-14 | Plate and VIN are stored in upper case. Text fields are trimmed, and a blank text field becomes NULL. | backend/src/api/orders.rs:312-316; backend/src/api/mod.rs:156-160 | documented |
| ORD-15 | If `spec` is sent, it is written in the same transaction as the order. The spec `form` comes from the project type's `spec_form`, never from the request. A spec sent for a project type with no form (or no project type) fails with 400 "this project type has no build specification", and the order is not created. | backend/src/api/orders.rs:172-175,248-259,365-379; backend/migrations/0015_order_specs.sql:12-15,27-29; frontend/src/app/[locale]/orders/new/page.tsx:23-26 | documented |
| ORD-16 | The spec `defrost` value must be `automatic`, `manual` or `hot_gas`, and `fuel` must be `diesel`, `electric`, `lpg` or `engine_coolant`, otherwise 400. Decimal fields accept a comma. `atp_class` is upper-cased. The DB enforces the ranges: target_temp −40…120, insulation 0…500, compartments 1…5, heat_output > 0. A cooling spec carries no heater fields and a heating spec carries no cooling fields. | backend/src/api/orders.rs:203-246; backend/migrations/0015_order_specs.sql:33-60 | documented |
| ORD-17 | After a successful create, the web replaces the route with `/[locale]/orders/{id}` and invalidates the order lists. | frontend/src/app/[locale]/orders/new/page.tsx:27-30 | implied |

### Order editing and line items

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| ORD-18 | PATCH semantics: an omitted field is kept, and an explicit `null` clears it. | docs/API.md:25; backend/src/api/mod.rs:162-186 | documented |
| ORD-19 | Changing `partner_id` without naming `contact_id` clears the contact. | backend/src/api/orders.rs:531,601-606 | documented |
| ORD-20 | While the order has at least one line item, changing its currency is refused with 422 `currency_locked`. The DB FK `(order_id, currency) … ON UPDATE RESTRICT` is the backstop. The web disables the currency select while items exist. | backend/src/api/orders.rs:537,636-641; backend/migrations/0003_orders.sql:64-66; docs/DECISIONS.md:33-34; frontend/src/components/forms/OrderForm.tsx:260,274,479-483; backend/tests/orders.rs:144 | documented |
| ORD-21 | Every successful PATCH writes one audit row `update` whose `changes` holds only the fields that changed, as `{field: [old, new]}`. It is written in the same transaction as the change. | backend/src/api/orders.rs:648-739; backend/src/repo/audit.rs:6-7,64-77 | documented |
| ORD-22 | Line-item rules: the description is non-blank (trimmed), and the quantity must be > 0, have at most 3 decimals, and be < 1,000,000,000. The unit price is in minor units and may be negative (discount lines). The item currency always equals the order currency. Violations return 400. | backend/src/domain/order.rs:46-66,88-110; backend/src/api/orders.rs:134-135,909; backend/migrations/0003_orders.sql:57-60 | documented |
| ORD-23 | The line total is round(quantity × unit_price), half away from zero, and must agree between Rust `Money` and SQL. The detail endpoint logs an error on mismatch but still returns 200. | backend/src/api/orders.rs:403,472-484; docs/DECISIONS.md:36-38 | documented |
| ORD-24 | Adding, updating or deleting an item locks the order row and writes an audit row on the order: `item_add` (id, description, quantity, unit_price), `item_update` (id plus diff), or `item_delete`. An added item with no `position` goes to the next position. | backend/src/api/orders.rs:927-961,990-1048,1064-1078 | documented |
| ORD-25 | Line items can be added, edited and deleted in any stage, including terminal ones. No stage-based lock exists in code. | backend/src/api/orders.rs:914-1081 (no stage check) | assumed |
| ORD-26 | `GET /orders/{id}/notes` returns imported MiniCRM activity. It is read-only and empty for orders created in AutoCRM. | backend/src/api/orders.rs:795-810; backend/migrations/0009_order_notes.sql:1-6 | documented |

### Intake slip (átvételi lap) and extras

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| ORD-27 | `mileage_in` is a non-negative integer or NULL. A negative value gets 400 "mileage_in cannot be negative" on both create and PATCH, and the DB CHECK agrees. | backend/src/api/orders.rs:321-326,623-629; backend/migrations/0018_intake_slip.sql:5 | documented |
| ORD-28 | `fuel_level` is one of `E`, `1/4`, `1/2`, `3/4`, `F`, or NULL. Any other value gets 400 "fuel_level must be one of E, 1/4, 1/2, 3/4, F", never a 500. | backend/src/api/orders.rs:261-272,631; backend/migrations/0019_intake_extras.sql:4-5 | documented |
| ORD-29 | `key_count` is ≥ 0 or NULL. Zero is a valid answer. A negative value gets 400 "key_count cannot be negative". | backend/src/api/orders.rs:289-295; backend/migrations/0019_intake_extras.sql:6-7 | documented |
| ORD-30 | `valuables_declared` is three-state: NULL means "not recorded", `false` means "recorded, nothing in the vehicle", and `true` means "recorded, and `valuables` says what". A `valuables` text without `valuables_declared = true` gets 400 "valuables requires valuables_declared = true". On PATCH, both values are resolved together, so unticking the box and clearing the text in one request succeeds rather than tripping the CHECK. | backend/migrations/0019_intake_extras.sql:9-16; backend/src/api/orders.rs:166-171,274-287,591-596; backend/tests/orders.rs:759 | documented |
| ORD-31 | The web intake slip is an inline form while empty. Its empty-state text says recording the slip is mandatory to close intake ("Az átvétel lezárásához kötelező"). Mileage and key count are validated client-side as non-negative integers. | frontend/src/components/orders/IntakeSlipSection.tsx:4-7,48-56; hu.json:269-270,273 | implied |

### Stage machine: configuration

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| ORD-32 | Stages are configuration (`stage_definitions`). Code refers only to stage keys, never to labels. Keys are permanent, and labels can be renamed. The only keys code depends on are lead `won` and order `intake` and `completed`. | backend/src/domain/stage.rs:3-4,35-43; backend/migrations/0002_partners_leads.sql:48,69; docs/API.md:159 | documented |
| ORD-33 | Seeded order pipeline, as key (position): `intake` (10, stall 7d), `design` (20, stall 21d), `production` (30, stall 30d), `meo` (40, gate: ≥1 `completion` image, stall 7d), `completed` (50, terminal), `cancelled` (60, terminal and exit). | backend/migrations/0002_partners_leads.sql:77-83; backend/src/domain/stage.rs:184-202 | documented |
| ORD-34 | DB invariants: an exit stage is always terminal; `min_images > 0` requires a `required_image_category`; `stall_after_days` is > 0 or NULL. Every `order_stages.stage_key` must exist in `stage_definitions` for entity `order` (composite FK). | backend/migrations/0002_partners_leads.sql:57-66; backend/migrations/0003_orders.sql:71-80; docs/DECISIONS.md:40-42 | documented |
| ORD-35 | The current stage is the latest `order_stages` row by `(entered_at DESC, id DESC)`. History is append-only, and each row's `left_at` is the next row's `entered_at`. Entries use `clock_timestamp()` so concurrent commits keep their order. | backend/src/repo/stages.rs:1,25-43,62-73,88-100; docs/DECISIONS.md:44-45 | documented |

### Stage machine: transitions

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| ORD-36 | A target key that is not defined is rejected with 422 `invalid_transition` "unknown stage '<key>'". An inactive target gets 422 `invalid_transition` "stage '<key>' is not active". A move to the current stage gets 422 `invalid_transition` "already in stage '<key>'". | backend/src/domain/stage.rs:72-77,108-118,327-342; backend/src/error.rs:78-87 | documented |
| ORD-37 | An order sitting in a stage that has since been deactivated can still move on. | backend/src/domain/stage.rs:113,343-347 | documented |
| ORD-38 | Forward moves (higher position, non-exit target) need no note and may skip stages. Every active, non-exit gate whose position is ≥ the current position and < the target position must be met. The current stage's own gate therefore applies when leaving it, and entering a gated stage is not gated. Skipping `meo` (for example `production` → `completed`) does not bypass the MEO photo gate. | backend/src/domain/stage.rs:7-8,140-161,260-276; docs/DECISIONS.md:54-55; frontend hu.json:329 ("A kihagyott fázisok képfeltételei is érvényesek.") | documented |
| ORD-39 | An unmet gate is refused with 422 `stage_gate`, message "stage '<gate key>' requires at least <n> '<category>' image(s) before moving on; <have> uploaded". Only images of the required category count, so 50 `production` photos do not satisfy the `completion` gate. | backend/src/domain/stage.rs:80-86,226-258; backend/src/error.rs:81; backend/tests/orders.rs:50-99 | documented |
| ORD-40 | A deactivated gate stage no longer applies. | backend/src/domain/stage.rs:141,350-358 | documented |
| ORD-41 | A backward move (lower position, non-exit target, from a non-terminal stage) is allowed only with a note that is non-blank after trimming. Otherwise 422 `note_required` "moving backwards or reopening a finished record requires a note". Backward moves skip image gates. | backend/src/domain/stage.rs:9,78-79,120,132-138,278-293; docs/DECISIONS.md:56; backend/src/api/leads.rs:306 | documented |
| ORD-42 | An exit stage (`cancelled`) is reachable from any non-terminal stage with no note and no gates (kind `exit`). | backend/src/domain/stage.rs:10,129-131,295-306; docs/DECISIONS.md:57-58 | documented |
| ORD-43 | Leaving a terminal stage (`completed` or `cancelled`) to any other stage is a reopen (kind `reopen`). It requires a non-blank note (otherwise 422 `note_required`) and skips gates. | backend/src/domain/stage.rs:11,122-128,308-325; docs/DECISIONS.md:59 | documented |
| ORD-44 | Intake-slip gate: when the current stage is `intake` and the target is any other stage (forward, backward-impossible, or exit `cancelled`), the move is refused with 422 `intake_slip_missing` "leaving intake requires the intake slip (mileage_in); record it on the order first" while `orders.mileage_in` is NULL. The refusal writes nothing. | backend/src/service/stages.rs:131-145; backend/src/domain/stage.rs:39-40; backend/migrations/0018_intake_slip.sql:1-4; hu.json:683 | documented |
| ORD-45 | A successful move does all of the following in one transaction under a row lock on the order: (a) inserts an `order_stages` row with `entered_by` = the user and the note; (b) writes an audit row `stage_change` `{from, to, kind, note}`; (c) queues the customer mail (ORD-50…52). It responds 200 `{from, to, kind}` with kind ∈ `forward`/`backward`/`exit`/`reopen`. | backend/src/service/stages.rs:112-172; backend/src/api/orders.rs:744-762 | documented |
| ORD-46 | The request body is `{stage, note?}`. The stage key is trimmed. A note that is blank after trimming is treated as absent. | backend/src/api/orders.rs:757-759; backend/src/api/leads.rs:303-308; docs/API.md:83 | documented |
| ORD-47 | `GET /orders/{id}/transitions` lists every active stage except the current one, and each entry carries three flags: `manual` (always true for orders), `requires_note` (true exactly when a move without a note would fail with `note_required`), and `gates_met` (true when the move with a note would succeed). The values are computed by the same `check_transition` as the move itself. A missing order is 404. | backend/src/service/stages.rs:26-93; backend/src/api/orders.rs:764-777 | documented |
| ORD-48 | `GET /orders/{id}/stages` returns the history oldest first, with `entered_at`, `left_at`, who, and the note. A missing order gives 404. | backend/src/api/orders.rs:779-793; docs/API.md:84 | documented |
| ORD-49 | The order detail's `stage` block carries `key`, `label_hu`, `entered_at`, `days_in_stage` (whole days since entry) and `is_terminal`. | backend/src/api/orders.rs:390-397,491-499 | documented |

### Stage machine: automatic mail on stage change

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| ORD-50 | Entering `completed` queues the `order_ready_for_pickup` letter (subject "{{order.number}} elkészült – átvehető") instead of the generic stage mail, so the customer gets one letter, not two. The idempotency key is `pickup:{stage_row_id}`. | backend/src/service/stages.rs:157-160; backend/src/service/automation.rs:225-262; backend/migrations/0016_pickup_template.sql:1-14 | documented |
| ORD-51 | Any other move of kind `forward` queues `order_stage_changed` with idempotency key `stage:{stage_row_id}`. Backward, exit (cancellation) and reopen moves send no customer mail. | backend/src/service/stages.rs:157-163; backend/src/service/automation.rs:187-223; docs/DECISIONS.md:89-90,98 | documented |
| ORD-52 | Customer mail is queued only when both `settings.stage_change_notifications` and `settings.automatic_email_enabled` are on, and only if a recipient exists: the order contact's email, otherwise the partner's email. Otherwise it is silently skipped and the stage move still succeeds. If queuing fails, the stage move fails too (same transaction). | backend/src/service/automation.rs:169-185,195-204,234-243; backend/src/service/stages.rs:159-164 | documented |
| ORD-53 | The stalled-order alert goes to every `stalled_alert_recipients` address for each open (non-terminal) order whose current visit exceeds its stage's `stall_after_days`. It is sent at most once per order, per stage visit, per recipient, per ISO week, and only when automatic email is enabled. | backend/src/service/automation.rs:118-167; backend/src/repo/reports.rs:203-221; docs/DECISIONS.md:89-90 | documented |
| ORD-54 | Kill switch: while automatic mail is off, any automatic mail found by the sender is cancelled, not held, so turning it back on releases no backlog. | docs/DECISIONS.md:93-94; backend/tests/email_and_jobs.rs:173-206 | documented |

### Stage machine: what clients show

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| ORD-55 | The web stage dialog takes its transition rules only from `/transitions`. It lists only `manual` options and disables options with `gates_met = false`, labelled "(feltétel nem teljesül)". Exit and terminal targets get "(kilépés)" and "(vég)" suffixes. When the selected definition has a gate, it shows "min. {n} db {category} kép". Save stays disabled until a target is chosen, a note is present when `requires_note`, and gates are met. | frontend/src/components/forms/OrderStageDialog.tsx:28,45-52,115,145-165,183; hu.json:329-334 | implied |
| ORD-56 | The web applies the move optimistically (new stage, days_in_stage 0, a new history row). On error it rolls back and shows the catalogued message for the error code: `stage_gate` "A megrendelés nem léphet tovább…", `note_required` "A visszalépéshez indoklás szükséges.", `intake_slip_missing` "Az átvétel lezárásához előbb rögzítsd az átvételi lapot (km-óra).", `invalid_transition` "Ez a fázisváltás nem engedélyezett.". On success it closes and refetches the order, history, transitions and lists. | frontend/src/components/forms/OrderStageDialog.tsx:56-113; frontend/src/lib/api/errors.ts:38-41,73-80; hu.json:681-684 | implied |
| ORD-57 | The Android stage dialog lists every option. Disallowed options (`!manual` or `!gates_met`) are greyed out and not selectable, with the reason in red: "Hiányzik a kötelező fotó ehhez a fázishoz." or "Ez a fázis nem érhető el kézi váltással." The note field appears, marked "Indoklás (kötelező)", only when `requires_note` is set, and confirm needs a non-blank note. A `stage_gate` error shows the server's detail. On success the dialog closes and the order reloads. The app never computes transitions itself. | android/.../ui/common/StageDialog.kt:23-27,50,56-83; android/.../data/api/Dto.kt:233-256; android/.../ui/orders/OrderDetailScreen.kt:160-198 | implied |
| ORD-58 | The stage rail (web) shows active stages, plus the current stage even if inactive, ordered by position. "Done" means the stage appears in the history, not that it sits before the current position. A cancelled order therefore shows never-reached stages as grey, not ticked. | frontend/src/components/ui/StageRail.tsx:24-38; docs/history/VIABILITY.md:39; docs/history/AUDIT.md:170 | documented |
| ORD-59 | The pickup board (`/board`) is a read-only wall display. The top section lists up to 30 `completed` orders, newest entry first. The lower section lists open orders currently in `design`, `production` or `meo` with plate, number, title, stage label and days in stage. It auto-refreshes every 30 s and requires login. (Hardcoded keys are finding B2.) | frontend/src/components/board/PickupBoard.tsx:3-6,19,25-45 | implied |
| ORD-60 | The order search filter `open=true` returns only orders whose current stage is not terminal. `stage=<key>` filters by the current stage key. | backend/src/api/orders.rs:80-82,107,111; backend/src/repo/orders.rs:339 | documented |

### Blockers

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| BLK-01 | A blocker means "we are waiting on a thing from someone" and belongs to exactly one order. Creating, updating, resolving or reopening one requires `ManageBlockers` (admin/office/designer). Reading needs only authentication. | backend/src/api/blockers.rs:1,139,183,256,302; backend/src/domain/role.rs:58-60; backend/migrations/0003_orders.sql:83-87 | documented |
| BLK-02 | Create: `what` is required (trimmed, non-empty; 400 "what is required"). `nudge_enabled` defaults to true. `responsible_email` is normalized and must be a valid address (400 otherwise). A `responsible_partner_id` must exist (400 "responsible partner does not exist"). A missing order gives 404. The response is 201 with the blocker, and an audit row `blocker_add {blocker_id, what}` is written on the order. | backend/src/api/blockers.rs:81-169; docs/API.md:94 | documented |
| BLK-03 | Update (PATCH): omitted fields are kept and `null` clears them. The same validations apply. An audit row `blocker_update` with the diff is written on the order. | backend/src/api/blockers.rs:81,96-125,171-237 | documented |
| BLK-04 | Resolve: sets `resolved_at` = now, `resolved_by` and an optional `resolution_note`, only if the blocker is open. Resolving an already-resolved blocker gives 409 `already_resolved` "blocker is already resolved". Audit row: `blocker_resolve {blocker_id, note}`. | backend/src/api/blockers.rs:239-290; backend/src/repo/blockers.rs:156-172; hu.json:679 | documented |
| BLK-05 | Reopen: clears `resolved_at`, `resolved_by` and `resolution_note`, only if the blocker is resolved. Reopening an open blocker gives 409 `not_resolved` "blocker is not resolved". Audit row: `blocker_reopen {blocker_id}`. | backend/src/api/blockers.rs:292-335; backend/src/repo/blockers.rs:223-232 | documented |
| BLK-06 | `is_overdue` is true exactly when the due date is set, the blocker is unresolved, and `due_date` < the business "today" (not UTC). Clients must display the server flag and never recompute it. | backend/src/repo/blockers.rs:25-27,51; frontend/src/lib/utils/blockers.ts:8-9 | documented |
| BLK-07 | Ordering: an order's blockers come open first, then by due date (nulls last), then id. `GET /blockers` returns only open blockers, optionally for one responsible partner, by due date (nulls last). | backend/src/repo/blockers.rs:79,104-105; docs/API.md:93 | documented |
| BLK-08 | Open blockers never block a stage move. No stage rule reads blockers, and cancelling an order leaves its blockers open. Blockers only show as a count on the stage rail and in the stalled report. | backend/src/domain/stage.rs:101-162; backend/src/service/stages.rs:112-172; docs/history/VIABILITY.md:39; backend/src/repo/reports.rs:210 | implied |
| BLK-09 | Nudge decision: a blocker is due a nudge when it is unresolved, `nudge_enabled`, has a due date, business-today > due date, and it has never been nudged or was last nudged ≥ `nudge_interval_days` ago. The escalated template (`blocker_nudge_escalated`) is used once `nudge_count ≥ nudge_escalate_after`, otherwise `blocker_nudge_first`. | backend/src/domain/blocker.rs:27-51,86-181; backend/src/service/automation.rs:68-72 | documented |
| BLK-10 | The nudge recipient is `responsible_email`, else the responsible partner's email. A blocker with no recipient is skipped with a warning. Nudges only run when automatic email is on. | backend/src/repo/blockers.rs:191-196; backend/src/service/automation.rs:22-27,52-58; backend/migrations/0003_orders.sql:89 | documented |
| BLK-11 | Each nudge number is queued at most once (key `nudge:{blocker_id}:{n}`). The count is re-checked under a row lock, so concurrent workers cannot double-send. A queued nudge increments `nudge_count`, sets `last_nudged_at`, and writes a system audit row `blocker_nudge {blocker_id, email_id, nudge, escalated}` on the order. | backend/src/domain/blocker.rs:53-57; backend/src/service/automation.rs:60-104; backend/src/repo/blockers.rs:203-221; docs/DECISIONS.md:89-92 | documented |
| BLK-12 | Web shows blockers read-only on the order tab ("Az akadályok itt csak megtekinthetők…"). Android implements create, resolve (with an optional note) and reopen. (Asymmetry is finding C2.) | frontend/src/app/[locale]/orders/[id]/page.tsx:546-556; hu.json:335; android/.../ui/orders/OrderDetailScreen.kt:238-292,547-596 | documented |

### Tasks (Feladatok)

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| TASK-01 | A task is a reminder pinned to an `order`, `lead` or `partner`. It has only two states, open and done (`done_at`), and no dependencies. | backend/src/repo/tasks.rs:1-6; backend/migrations/0017_tasks.sql:1-18 | documented |
| TASK-02 | Any authenticated user may create, toggle and delete any task. No capability gate applies, and `created_by` records the creator. | backend/src/api/tasks.rs:1-6,141-167; backend/src/repo/tasks.rs:5-6 | documented |
| TASK-03 | Create: `title` is trimmed and must be 1–200 characters (400 "title is required" / "title is at most 200 characters"). `entity_type` must be order/lead/partner and the record must exist (400 "the pinned record does not exist"). `assigned_to`, if given, must be an existing user (400 "assigned_to is not a user"). The due date is optional. The response is 201. | backend/src/api/tasks.rs:54-108; backend/migrations/0017_tasks.sql:6-12 | documented |
| TASK-04 | `POST /tasks/{id}/done {done}` sets `done_at` = now when true and clears it when false. An unknown id gives 404. Delete returns 204, or 404 when the id is unknown. | backend/src/api/tasks.rs:130-167; backend/src/repo/tasks.rs:89-111 | documented |
| TASK-05 | `GET /tasks` returns the caller's open tasks: assigned to them or created by them, ordered by due date (nulls last), then newest. `GET /tasks/for/{entity}/{id}` returns everything pinned to a record: open first, then by due date, then newest. | backend/src/api/tasks.rs:32-42,110-128; backend/src/repo/tasks.rs:54-86 | documented |
| TASK-06 | When an assignee user is deleted, the task's `assigned_to` becomes NULL and the task is kept. | backend/migrations/0017_tasks.sql:11 | documented |
| TASK-07 | Task writes produce no audit rows and do not affect stages. | backend/src/api/tasks.rs:80-167 (no audit call) | implied |

### Order audit trail

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| ORD-61 | Every order mutation (create, update, spec_set, item_add, item_update, item_delete, stage_change, blocker_add, blocker_update, blocker_resolve, blocker_reopen, blocker_nudge) writes an `audit_log` row with entity `order` in the same transaction as the change. The trail can never disagree with the data. | backend/src/repo/audit.rs:6-7; backend/src/api/orders.rs:370-377,739,851-859,953-961,1040-1048,1070-1078; backend/src/service/stages.rs:148-156; backend/src/api/blockers.rs:151-158,226-234,272-280,317-325; backend/src/service/automation.rs:94-101 | documented |
| ORD-62 | `GET /orders/{id}/audit` returns rows newest first with the actor's display name. `limit` defaults to 50 and is clamped to 1..200. The web requests 100 and shows them grouped by day on the "audit" tab. System actions have `user_id` NULL. | backend/src/api/orders.rs:864-884; backend/src/repo/audit.rs:42-62; backend/src/api/mod.rs:137-139; frontend/src/app/[locale]/orders/[id]/page.tsx:143-146,625-635 | documented |

### Android order picker and sticky capture order

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| ORD-63 | `GET /mobile/orders` returns at most 200 compact rows (id, number, title, partner, "make model", plate, stage key and label) with `Cache-Control: private, max-age=60`. It shows open (non-terminal) orders by default, or all orders with `all=true`. `q` matches number, title, partner, VIN, and the plate ignoring spaces, dashes and case. | backend/src/api/mobile.rs:25-89; backend/src/domain/order.rs:24-32; docs/API.md:125 | documented |
| ORD-64 | In the working tree, the picker is wired as the Capture tab. Choosing an order stores it as the device's sticky current order (id, number, title, plate), which persists across sessions, and navigates to `capture/{id}`. The picker shows the last chosen order for one-tap reuse. Search is debounced by 300 ms, and a toggle includes finished orders. Offline, it reports "Nincs kapcsolat. A már kiválasztott megrendelésen tudsz fotózni." (E1/E2 describe the pre-change state.) | android/.../MainActivity.kt:350-364; android/.../ui/picker/OrderPickerScreen.kt:55-139; android/.../data/prefs/CapturePrefs.kt:13-20,38-46,57-71 | implied |
| ORD-65 | The sticky photo category defaults to `production`, never `intake`. Manually attached photos are production-only. Any other stored value is coerced back to `production`, because intake photos are immutable evidence with their own flow. | android/.../data/prefs/CapturePrefs.kt:48-55,90-95,106-107; android/.../ui/photos/OrderPhotoSection.kt:85-96 | documented |

<a id="inspection"></a>

## Inspection & media

Scope: the vehicle handover inspection (átadás-átvétel): check-out ("Kiadás") and check-in ("Visszavétel") walkarounds, the comparison between them, signing and locking, and post-sign notes. Also photo capture on Android, the durable offline upload queue, the ticket → PUT → complete upload protocol, the image-processing pipeline, and media storage. Entry points: Android `InspectionHomeScreen` (per order, "Kiadás indítása" / "Visszavétel") → `WalkaroundScreen` (phases Readings → Zone → Damage → Comparison (check-in only) → Summary → Signing → Done) → `InspectionSyncWorker`; Android `CaptureScreen` / `OrderPhotoSection` ("Kamera" / "Galéria") → `UploadQueue` → `UploadWorker`; Android `QueueScreen` ("Feltöltési sor"); web `InspectionSection` inside the order's Átvételi lap (read, verdicts, notes; no create); web settings `ZoneTemplates`. API: `/inspections…`, `/orders/{id}/uploads`, `/uploads/complete`, `/orders/{id}/images`, `/images/{id}/original`, `DELETE /images/{id}`, `/mobile/orders`. Key files: `backend/migrations/0004_media.sql`, `0025_image_category_inspection.sql`, `0026_inspections.sql`; `backend/src/api/{inspections,media,mobile}.rs`; `backend/src/repo/{inspections,images}.rs`; `backend/src/service/media.rs`; `backend/src/service/automation.rs` (`process_image`); `backend/src/media/{upload_token,pipeline,storage}.rs`; `backend/src/domain/media.rs`; Android (`android/app/src/main/java/hu/autotherm/autocrm/`, abbreviated `A:` below) `data/upload/*`, `data/db/{PendingUpload,PendingUploadDao,InspectionDraft}.kt`, `data/inspection/*`, `ui/inspection/*`, `ui/photos/OrderPhotoSection.kt`, `ui/queue/QueueScreen.kt`, `data/prefs/CapturePrefs.kt`; web `frontend/src/components/inspections/InspectionSection.tsx`. Docs: `docs/DECISIONS.md` §Images, `docs/API.md` §Images & documents.

Error envelope referenced below (backend/src/error.rs:97-143): `Rule` → 422 with its code; `Validation` → 400 `validation`; `NotFound` → 404 `not_found`; `Forbidden` → 403; `Conflict` → 409; unique violation 23505 → 409 `duplicate`; FK 23503 → 422 `invalid_reference`; trigger AC001 → 409 `immutable`. Android maps 401 → `Unauthenticated`, 403 → `Forbidden`, 404 → `NotFound`, 400/409/422 → `Rule(code)`, everything else → `Server`; only `Network` and `Server` are retryable (A:data/api/AutoCrmApi.kt:133-146; A:data/api/ApiError.kt:36-38).

Related existing findings (not re-reported): C1 (phone photos invisible on web), B3 (duplicated PUT on Android), D3 (offline only on Android), E2 (sticky order write-only), E3 (walkaround `comparison` field / comparison endpoint unused on Android).

### Inspection lifecycle: creation, permissions, listing

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| INSP-01 | An inspection belongs to exactly one order; `kind` ∈ {`checkout`,`checkin`}, `status` ∈ {`draft`,`signed`}; a new row starts as `draft`. Any other `kind` on create → 400 "kind must be checkout or checkin". | backend/migrations/0026_inspections.sql:1-3, 13-15; backend/src/api/inspections.rs:153-155 | documented |
| INSP-02 | Reads (list, detail, comparison, templates) need only authentication; every inspection mutation (create, patch, delete, photos, damages, signatures, sign, notes, verdicts) needs `ChangeStages` (roles Admin, Office, Designer); replacing zone templates needs `ManageConfiguration` (Admin only). Missing capability → 403. | backend/src/api/inspections.rs:12-14, 152, 349, 403, 442, 509, 551, 580, 614, 662, 758, 854; backend/src/domain/role.rs:49-59 | documented |
| INSP-03 | Creating an inspection for a non-existent order → 404 `order`. | backend/src/api/inspections.rs:156-158 | documented |
| INSP-04 | `inspector_name` is required (non-blank after trim), 1..200 chars. On Android the walkaround cannot leave the Readings step until "Átadó / felvevő neve *" is filled (error "az átadó neve kötelező"); it defaults to the signed-in account's display name. | backend/src/api/inspections.rs:159; backend/migrations/0026_inspections.sql:18; A:ui/inspection/WalkaroundScreen.kt:173, 278-283; A:ui/inspection/WalkaroundViewModel.kt:153-157 | documented |
| INSP-05 | `vehicle_plate` defaults to the order's plate when omitted or blank; if neither exists → 400 "vehicle_plate is required". Plate 1..32 chars. `vehicle_vin` likewise defaults to the order's VIN. | backend/src/api/inspections.rs:192-197; backend/migrations/0026_inspections.sql:16-17 | documented |
| INSP-06 | `odometer` must be ≥ 0 and `battery_pct` 0..100 on both create and patch (400 otherwise; DB CHECK as backstop). Android accepts digits only for both and caps battery input at 3 digits. | backend/src/api/inspections.rs:160-169, 351-360; backend/migrations/0026_inspections.sql:21-23; A:ui/inspection/WalkaroundScreen.kt:229-263 | documented |
| INSP-07 | Fuel level is free text 1..16 chars server-side; the Android UI offers exactly `E`, `1/4`, `1/2`, `3/4`, `F` (tapping the selected chip clears it). | backend/migrations/0026_inspections.sql:22; A:ui/inspection/WalkaroundScreen.kt:242-254 | implied |
| INSP-08 | An order may have at most one `draft` check-out. A second draft check-out → 422 `checkout_open` ("this order already has an open check-out; sign or discard it first"). | backend/migrations/0026_inspections.sql:38-41; backend/src/api/inspections.rs:210-222; backend/tests/inspections.rs:59-64 | documented |
| INSP-09 | A check-in can be created only when the order has ≥1 `signed` check-out; otherwise 422 `checkout_required`. Its `checkout_id` is set automatically to the order's latest signed check-out (`signed_at DESC NULLS LAST, id DESC`); the client cannot choose it. The DB forbids a check-in without `checkout_id`. | backend/src/api/inspections.rs:170-186; backend/src/repo/inspections.rs:126-143; backend/migrations/0026_inspections.sql:25-27, 33; backend/tests/inspections.rs:125-130 | documented |
| INSP-10 | `GET /inspections` requires `order_id` or a non-blank `plate` (else 400); unknown `order_id` → 404. Rows are newest first (`created_at DESC, id DESC`). `plate` returns the vehicle's history across all orders (exact plate match). | backend/src/api/inspections.rs:96-124; backend/src/repo/inspections.rs:103, 119 | documented |
| INSP-11 | Inspections are created only on the phone (guided walkaround); the web has deliberately no create button. | backend/src/api/inspections.rs:5-6; frontend/src/components/inspections/InspectionSection.tsx:3-6 | documented |
| INSP-12 | Only users who can inspect see "Kiadás indítása" / "Visszavétel" on Android; everyone sees the phone's local drafts ("Folyamatban a telefonon") and server history ("Előzmények") with badge "Lezárva" (signed) or "Piszkozat" (draft). | A:ui/inspection/InspectionHomeScreen.kt:148-161, 162-198, 199-224 | implied |

### Draft editing and discard

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| INSP-13 | `PATCH /inspections/{id}` only works on a draft. Omitted fields keep their value; an explicit `null` or blank string clears an optional field; `inspector_name` can be changed but never cleared (blank is ignored). Plate/VIN/kind/order are not patchable. | backend/src/api/inspections.rs:325-391; backend/src/repo/inspections.rs:145-171 | implied |
| INSP-14 | A draft can be discarded (`DELETE /inspections/{id}` → 204), cascading its damages, photo links, verdicts, signatures and notes. A signed inspection cannot: 422 `locked` ("a signed inspection cannot be discarded"). A missing id → 404. | backend/src/api/inspections.rs:398-416; backend/src/repo/inspections.rs:206-212; backend/migrations/0026_inspections.sql:45, 61, 75, 90, 103 | documented |
| INSP-15 | Every mutation on a signed inspection (patch, photo attach, damage add/remove, signature, sign, verdict) → 422 `locked` ("a signed inspection cannot be changed; add a follow-up note instead"). The repo layer re-checks `status = 'draft'` in the UPDATE/DELETE `WHERE`. | backend/src/api/inspections.rs:76-90; backend/migrations/0026_inspections.sql:8-9; backend/src/repo/inspections.rs:6-7, 168, 195, 208; backend/tests/inspections.rs:114-123 | documented |

### Zones, damages and inspection photos

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| INSP-16 | Zone templates: set `default` always applies; set `cooling` is added when the order's project type has `spec_form = 'cooling'`. Result is ordered by `position, id`. Seed: 15 default zones (only `roof` optional) + 3 cooling zones (`cargo_box`, `cargo_doors`, `refrigeration_unit`). | backend/migrations/0026_inspections.sql:110-142; backend/src/api/inspections.rs:811-826; backend/src/repo/inspections.rs:521-534; backend/tests/inspections.rs:194-206 | documented |
| INSP-17 | `PUT /inspections/templates/{set}` replaces a whole set atomically (delete + insert in one transaction); `set` must be `default` or `cooling` (400 otherwise); 1..60 zones; `zone_key` 1..64 and `instruction` 1..300 required; `(set_key, zone_key)` unique (duplicate → 409 `duplicate`). | backend/src/api/inspections.rs:848-875; backend/migrations/0026_inspections.sql:113-122 | documented |
| INSP-18 | The phone downloads the resolved zone list at inspection start and freezes it in the draft; when the server is unreachable, it uses a built-in 15-zone fallback identical to the default seed (roof optional). | backend/migrations/0026_inspections.sql:111-112; A:ui/inspection/WalkaroundViewModel.kt:37-54, 158-175 | documented |
| INSP-19 | A damage has `damage_type` ∈ {scratch, dent, crack, chip, broken, missing, stain, tear, other}, `severity` ∈ {minor, moderate, severe}, `view` ∈ {top, side} (default `top`), optional marker `x`,`y` each in [0,1]; anything else → 400. | backend/src/api/inspections.rs:45-48, 511-523; backend/migrations/0026_inspections.sql:47-54 | documented |
| INSP-20 | On Android a damage cannot be finished until both type and severity are chosen ("válassz típust és súlyosságot"). "Eldobás" on a damage removes it and its close-up photos (files and queue rows). | A:ui/inspection/WalkaroundScreen.kt:524-535; A:ui/inspection/WalkaroundViewModel.kt:381-405 | implied |
| INSP-21 | Removing a damage (`DELETE …/damages/{damage_id}`) only works on a draft and only for a damage of that inspection (else 404 `damage`); photos pointing at it keep existing with `damage_id = NULL`. | backend/src/api/inspections.rs:546-557; backend/src/repo/inspections.rs:336-349; backend/migrations/0026_inspections.sql:65 | documented |
| INSP-22 | Attaching a photo requires: draft inspection; `purpose` ∈ {overview, closeup, dashboard, signature}; non-blank `zone_key`; an existing, non-deleted image of the **same order** with category `inspection`; optional `damage_id` belonging to this inspection. Violations → 400 (404 for missing image). | backend/src/api/inspections.rs:436-463; backend/migrations/0026_inspections.sql:59-70 | documented |
| INSP-23 | Each image attaches to at most one inspection, ever; a second attach → 422 `already_attached`. | backend/migrations/0026_inspections.sql:62; backend/src/api/inspections.rs:450-451, 476-482; backend/tests/inspections.rs:96-100 | documented |
| INSP-24 | Inspection photos may only come from the inspection camera loop: never from the gallery, file picker or the ad-hoc "Kamera" button. The manual capture UI never offers the `inspection` category. | A:ui/inspection/CameraCapture.kt:56-63; A:data/prefs/CapturePrefs.kt:83-95; A:ui/photos/OrderPhotoSection.kt:44-46 | documented |
| INSP-25 | Walkaround: for every zone the camera auto-opens once when the zone has no overview; cancelling returns to the zone step and does not reopen. After an overview exists the user answers "Van sérülés ebben a zónában?" (Igen → new damage + close-up capture; Nem → next zone). | A:ui/inspection/WalkaroundScreen.kt:305-315, 343-363 | implied |
| INSP-26 | Every capture is shown for review ("Újra" / "Megtartom"). "Újra" deletes the file and its queue row and removes the unattached photo entry, then reopens the camera for the same zone/purpose/damage. | A:ui/inspection/WalkaroundViewModel.kt:309-346; A:ui/inspection/WalkaroundScreen.kt:606-631 | implied |
| INSP-27 | Each photo records `taken_at` (phone clock at capture, RFC 3339) and best-effort GPS (`lat`/`lon` from last known location, only with location permission); missing location never blocks capture. | A:ui/inspection/WalkaroundViewModel.kt:217-238, 281-292; backend/migrations/0026_inspections.sql:66-68 | documented |
| INSP-28 | Inspection photos are mutable image rows (not write-once like intake): "drafts are retaken freely, and the lock lives on the inspection status rather than on the image row". | backend/src/domain/media.rs:15-18, 32-36; backend/migrations/0025_image_category_inspection.sql:5-8 | documented |
| INSP-29 | Detail view returns photos with presigned `thumb_url` / `display_url` valid 1 h, both `null` for a deleted image or before derived copies exist; verdicts are returned only for check-ins (empty list for check-outs). | backend/src/api/inspections.rs:42, 238-241, 254-307 | documented |

### Check-in comparison and verdicts

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| INSP-30 | `GET /inspections/{id}/comparison` is valid only for a check-in (400 "only a check-in has a comparison" otherwise) and returns both full details plus one suggestion per check-in damage: same `zone_key` + same `damage_type` as some check-out damage → `preexisting` (with that check-out damage id), otherwise `new`. Suggestions are advisory; the inspector still decides. | backend/src/api/inspections.rs:674-735 | documented |
| INSP-31 | Verdicts exist only on check-ins (400 "only a check-in takes verdicts"); `verdict` ∈ {preexisting, new, dismissed}; `checkin_damage_id` must belong to this check-in; an optional `checkout_damage_id` must belong to the linked check-out. Violations → 400. | backend/src/api/inspections.rs:752-780; backend/migrations/0026_inspections.sql:73-85 | documented |
| INSP-32 | Every check-in damage has exactly one verdict: setting it again overwrites it (upsert on `checkin_damage_id`, updating `reviewed_by` and `reviewed_at`). | backend/migrations/0026_inspections.sql:73-74, 78; backend/src/repo/inspections.rs:366-397 | documented |
| INSP-33 | Android: after the last zone a check-in goes to Comparison (check-out goes straight to Summary). Each check-in damage shows "Átadáskor is megvolt" or "Újnak tűnik", the check-in close-up next to the check-out overview of the same zone, and chips "Megvolt" / "Új" / "Nem sérülés"; the Continue button shows how many decisions remain. Comparison needs network ("Az átadás adataihoz jel kell"); skipping is allowed but signing will still require it. | A:ui/inspection/WalkaroundViewModel.kt:411-425; A:ui/inspection/InspectionFinishScreens.kt:41-45, 57-83, 110-190 | implied |
| INSP-34 | Web: a check-in shows the comparison with suggestion badge, current verdict and the matched check-out damage; users with `ChangeStages` get verdict buttons that send the suggested `checkout_damage_id`; errors are shown inline. The odometer delta (check-in − check-out) is shown when both readings exist. | frontend/src/components/inspections/InspectionSection.tsx:217, 242-357 | implied |

### Signing and immutability

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| INSP-35 | Signatures: `role` ∈ {inspector, customer}; `name` required 1..200; `document_id` must be a live document of the same order (the finger-drawn PNG uploaded as a document). At most one signature per role; re-signing a role on a draft replaces name/document and resets `signed_at`. | backend/src/api/inspections.rs:559-595; backend/src/repo/inspections.rs:427-448; backend/migrations/0026_inspections.sql:88-97 | documented |
| INSP-36 | `POST /inspections/{id}/sign` succeeds only when both an `inspector` and a `customer` signature exist; otherwise 422 `signatures_required`. | backend/src/api/inspections.rs:615-623 | documented |
| INSP-37 | A check-in signs only when every damage has a verdict; otherwise 422 `verdicts_pending` "{n} damage item(s) still need a review verdict". A check-out has no verdict requirement. | backend/src/api/inspections.rs:624-638 | documented |
| INSP-38 | Signing sets `status = 'signed'`, `signed_at = now()` and, if given and non-blank, `customer_comment`; a signed row must have `signed_at`. Signing is one-way: no endpoint returns a signed inspection to draft. | backend/src/repo/inspections.rs:186-200; backend/migrations/0026_inspections.sql:34; backend/src/api/inspections.rs:76-90 | documented |
| INSP-39 | Before local sign-off the phone requires: an `overview` photo for every non-optional zone (else "hiányzó zónafotó: …" and it jumps to the first missing zone); both signatures drawn and both printed names filled ("mindkét aláírás és név kötelező"); for a check-in, a verdict for every damage. | A:ui/inspection/WalkaroundViewModel.kt:502-539; A:ui/inspection/InspectionFinishScreens.kt:355-380 | documented |
| INSP-40 | The signing screen tells both parties that after signing the inspection is locked and only separate time-stamped notes can be added. After local sign-off the draft shows "aláírva, feltöltésre vár" until synced and the screen shows "Átvétel lezárva". | A:ui/inspection/InspectionFinishScreens.kt:299-304, 386-395; A:ui/inspection/InspectionHomeScreen.kt:112 | implied |
| INSP-41 | Notes (`POST …/notes`) are the only way to add information after signing: body required, ≤ 2000 chars (400 otherwise), timestamped, attributed to the author, never edited or deleted (no update/delete endpoint). Notes need `ChangeStages`. | backend/migrations/0026_inspections.sql:99-106; backend/src/api/inspections.rs:645-672; backend/tests/inspections.rs:186-190 | documented |
| INSP-42 | Notes are offered on both clients only to users who can change stages ("Időbélyegzett megjegyzés" on Android; NoteComposer hidden otherwise on web); a blank note cannot be submitted. | A:ui/inspection/InspectionHomeScreen.kt:266-279, 326-348; frontend/src/components/inspections/InspectionSection.tsx:380-400 | implied |
| INSP-43 | A signed inspection "lives forever": signed rows cannot be discarded, and a signature document cannot be hard-deleted while referenced (`ON DELETE RESTRICT`). | backend/src/repo/inspections.rs:206; backend/migrations/0026_inspections.sql:93 | documented |

### Offline inspection drafts and sync (Android)

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| INSP-44 | The whole walkaround (readings, zone order, photo refs by sha256, damages, verdicts, signatures, `signed` flag) is persisted to `inspection_drafts.payload_json` after every mutation, so a dead battery resumes exactly where it stopped. A resumed signed draft opens on Done; an unsigned one on the Zone step. | A:data/db/InspectionDraft.kt:12-21; A:data/inspection/DraftModels.kt:9-16, 62-86; A:ui/inspection/WalkaroundViewModel.kt:112-145 | documented |
| INSP-45 | Inspection photo and signature files live in app-private `files/inspections/{uuid}/`, never in the gallery; inspection photo bytes also get a `pending_uploads` row with category `inspection`. | A:data/inspection/InspectionSync.kt:25-27; A:ui/inspection/WalkaroundViewModel.kt:260-280 | documented |
| INSP-46 | Sync pushes a draft in dependency order: create inspection → damages → photos (upload then attach) → signatures (PNG uploaded as `other` document) → verdicts → sign; each step records its server id in the payload so a retry resumes without duplicates. | A:data/inspection/InspectionSync.kt:38-42, 57-225 | documented |
| INSP-47 | An offline-started check-in must verify a signed check-out exists before the server create; if none, the draft fails visibly ("nincs lezárt átadás ehhez az összehasonlításhoz"). | A:data/inspection/InspectionSync.kt:60-70; A:ui/inspection/WalkaroundViewModel.kt:176-186 | documented |
| INSP-48 | If an attach returns `already_attached` (previous sync died between attach and persist), sync recovers the attached photo id from the server detail instead of failing. | A:data/inspection/InspectionSync.kt:174-186 | documented |
| INSP-49 | Network/5xx errors leave the draft in `draft` and retry later; non-retryable errors (4xx rules, missing files, blocked photo upload) record a visible error on the draft; a draft is never dropped silently. | A:data/inspection/InspectionSync.kt:29-36, 231-252; A:data/db/InspectionDraft.kt:19-21; A:data/inspection/InspectionSyncWorker.kt:22-25 | documented |
| INSP-50 | Only after the server sign succeeds is the local draft deleted together with its file directory ("Server history is the record"). | A:data/inspection/InspectionSync.kt:225-230 | documented |
| INSP-51 | Inspection-category queue rows are owned by the inspection sync: the general upload worker skips and never sweeps them, and the queue screen offers no retry/discard for them ("Átvételi fotó – az átvételnél kezelendő."). | A:data/upload/UploadWorker.kt:44-48, 71-78; A:ui/queue/QueueScreen.kt:148-157 | documented |
| INSP-52 | Discarding a local draft ("Eldobás" on the home screen) deletes the draft row, its queued photo rows and files, and its directory. | A:data/inspection/InspectionSyncWorker.kt:77-98; A:ui/inspection/InspectionHomeScreen.kt:98-102, 185-187 | documented |

### Capture and the offline upload queue (Android)

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| MEDIA-01 | Manual photos are added from inside a job only (order already chosen), via the system camera app ("Kamera") or the system photo picker ("Galéria", images only, max 20 per pick); both deliver full-quality originals. | A:ui/photos/OrderPhotoSection.kt:41-61, 158-162, 212-234; git 701a0ec message | documented |
| MEDIA-02 | Manually attached photos are always filed as `production` ("Gyártás"); a stale sticky value of any other category is coerced back to production. Intake and handover photos come only from their own flows. UI text: "Ide gyártás közbeni fotók kerülnek. A bevételi és az átadási fotók a saját folyamatukban készülnek." | A:data/prefs/CapturePrefs.kt:48-55, 90-95; A:ui/photos/OrderPhotoSection.kt:85-96, 201-206 | documented |
| MEDIA-03 | If no camera app is available, the user sees "Nincs elérhető kameraalkalmazás ezen a telefonon." instead of a crash; a cancelled/empty camera result deletes the empty target file and queues nothing. | A:ui/photos/OrderPhotoSection.kt:121-137, 212-224 | documented |
| MEDIA-04 | A captured/picked photo is copied/moved into app-private `files/pending/` and its `pending_uploads` row is written before `enqueue` returns; only then is the worker started. Gallery items are copied, not referenced, so deleting them from the gallery cannot lose queued evidence. | A:data/upload/UploadQueue.kt:13-19, 44-75, 114-136; A:data/db/PendingUpload.kt:8-20 | documented |
| MEDIA-05 | The sha256 is computed once at capture (lowercase hex, streamed in 64 KB blocks) and never recomputed; it binds the ticket and makes retries idempotent. | A:data/db/PendingUpload.kt:43-44; A:data/upload/Uploader.kt:152-171 | documented |
| MEDIA-06 | Local dedupe: `(order_id, sha256)` is unique in `pending_uploads`; the same bytes for the same order enqueue one row (insert IGNORE, never REPLACE, so an in-flight row keeps its attempts and ticket). The user sees "N fotó sorba állítva. / N már sorban állt. / N nem olvasható." | A:data/db/PendingUpload.kt:26-28; A:data/db/PendingUploadDao.kt:13-21; A:data/upload/UploadQueue.kt:36-42, 58-64; A:ui/photos/OrderPhotoSection.kt:98-119; android/app/src/test/java/hu/autotherm/autocrm/UploadQueueTest.kt:177-197 | documented |
| MEDIA-07 | The same photo queued for two different orders gets two separate files (`{orderId}-{sha}.jpg`), so finishing one cannot delete the other's bytes. | A:data/upload/UploadQueue.kt:66-75 | documented |
| MEDIA-08 | An unreadable picked item (cloud-only, revoked permission, zero bytes) is reported as unreadable and not queued. | A:data/upload/UploadQueue.kt:40-41, 119-136 | documented |
| MEDIA-09 | Row states: `pending` → `uploading` → `done`, or `blocked` (permanent refusal). Blocked rows are kept, never auto-deleted; only the user's explicit "Eldobás" deletes queued bytes. | A:data/db/PendingUpload.kt:63-81; A:data/upload/UploadQueue.kt:102-111; A:data/db/PendingUploadDao.kt:96-98 | documented |
| MEDIA-10 | The worker processes due rows (`state IN ('pending','uploading') AND next_attempt_at <= now`) oldest first (`created_at, id`), up to 20 per batch, one at a time (no parallel uploads). Stranded `uploading` rows after process death are retried. | A:data/db/PendingUploadDao.kt:29-43; A:data/upload/UploadWorker.kt:24-28 | documented |
| MEDIA-11 | A retryable failure (network, 5xx, storage 403 from an expired presigned URL) increments `attempts`, clears the ticket, and schedules the next try after 0 s, 10 s, 1 min, 5 min, 30 min, then 2 h (capped). A pass that produced a retry stops and returns `Result.retry()` (WorkManager exponential backoff from 30 s). | A:data/db/PendingUpload.kt:83-94; A:data/db/PendingUploadDao.kt:84-94; A:data/upload/UploadWorker.kt:55-68, 80, 97; A:data/upload/Uploader.kt:125-131 | documented |
| MEDIA-12 | A non-retryable failure (400/409/422 rule, 404 order gone, 403 forbidden, 401, storage 4xx other than 403, file missing on phone) marks the row `blocked` with a Hungarian reason ("a munkamenet lejárt, jelentkezz be újra", "nincs jogosultság a feltöltéshez", "a megrendelés már nem létezik", "a fájl már nincs meg a telefonon", or the server's message). | A:data/upload/Uploader.kt:46-52, 96-101, 127-142; A:data/db/PendingUpload.kt:69-75; UploadQueueTest.kt:113-129 | documented |
| MEDIA-13 | Without a session the worker does nothing and succeeds; rows stay queued. Signing out keeps the queue ("photos belong to the job, not the session"). | A:data/upload/UploadWorker.kt:35-37; A:ui/queue/QueueScreen.kt:59-68 | documented |
| MEDIA-14 | The worker is started after every capture and at app start (`KEEP`: a burst does not restart the drain); "Újra" on the queue screen resets backoff for blocked and pending rows and restarts the drain immediately (`REPLACE`). It runs only with network connectivity. | A:data/upload/UploadWorker.kt:86-114; A:AutoCrmApp.kt:55-60; A:data/db/PendingUploadDao.kt:100-111; A:data/upload/UploadQueue.kt:92-100 | documented |
| MEDIA-15 | A still-valid ticket (more than 60 s before expiry) is reused on resume so already-PUT bytes go straight to complete instead of requesting a new ticket. | A:data/db/PendingUpload.kt:49-55, 97-98; A:data/upload/Uploader.kt:57-60 | documented |
| MEDIA-16 | `already_uploaded` from the ticket request marks the row done with the returned image id without any PUT. | A:data/upload/Uploader.kt:73-78; UploadQueueTest.kt:133-148 | documented |
| MEDIA-17 | Done rows remain briefly so the queue can show progress, then the worker deletes each row together with its file; a row and its file are always freed together. | A:data/db/PendingUpload.kt:77-81; A:data/db/PendingUploadDao.kt:115-129; A:data/upload/UploadWorker.kt:71-78 | documented |
| MEDIA-18 | Queue screen ("Feltöltési sor") lists all non-done rows newest first with order number, category label, time, size in kB, attempt count and last error; blocked rows show "Hiba" and "N fotó nem ment fel. Egyik sem veszett el — itt vannak a telefonon."; empty queue shows "Minden fotó feltöltve."; order screens show "N feltöltésre vár". | A:ui/queue/QueueScreen.kt:71-167; A:data/db/PendingUploadDao.kt:51; A:ui/photos/OrderPhotoSection.kt:197-199 | implied |

### Upload protocol, tickets and storage (backend)

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| MEDIA-19 | Upload is three steps: `POST /orders/{id}/uploads` (ticket + presigned PUT, or `already_uploaded`) → client PUTs directly to object storage with every listed header → `POST /uploads/complete {ticket}` → 201 new / 200 already recorded. Photo bytes never pass through the API server. | backend/src/service/media.rs:1-2, 38-53; backend/src/api/media.rs:87-120; docs/API.md:104-113 | documented |
| MEDIA-20 | Requesting and completing uploads need `UploadMedia` (Admin, Office, Designer); an unknown order → 404. Images belong only to orders; an image target on a lead → 400. | backend/src/api/media.rs:57, 76, 100; backend/src/service/media.rs:98-130; backend/src/domain/role.rs:57-59 | documented |
| MEDIA-21 | Request validation: `sha256` must be 64 hex chars (case-insensitive); `content_type` non-empty, ≤100 printable ASCII chars containing `/`; `byte_size` > 0; images ≤ 50 MiB and of type `image/jpeg`, `image/png`, `image/webp`, `image/heic` or `image/heif` (else 400 "unsupported image type (jpeg, png, webp, heic)"). Filenames are reduced to their basename, control chars stripped, ≤200 chars. | backend/src/service/media.rs:62-90, 110-136; backend/src/domain/media.rs:60-74 | documented |
| MEDIA-22 | Server dedupe: if a live image with the same `(order_id, sha256)` exists, the request returns `already_uploaded` with its id (regardless of the requested category); completing a second time returns the existing row with `created = false` (200). The DB enforces the uniqueness among non-deleted images. | backend/src/service/media.rs:137-142; backend/src/repo/images.rs:107-137; backend/migrations/0004_media.sql:30-32; docs/DECISIONS.md:77-78 | documented |
| MEDIA-23 | Image object keys are content-addressed: `orders/{order_id}/{category}/{sha256}.{ext}`; derived copies `orders/{order_id}/{category}/derived/{sha256}.{display\|thumb}.jpg`. | backend/src/domain/media.rs:84-104; docs/DECISIONS.md:67-70 | documented |
| MEDIA-24 | The presigned PUT signs content type, length and `x-amz-checksum-sha256`, so storage rejects any other body; for intake photos it also sets Object Lock (governance mode) until now + `S3_INTAKE_LOCK_YEARS`×365 + 2 days (none when the setting is 0). | backend/src/service/media.rs:143-148, 173-183; backend/src/media/storage.rs:72-76, 98-120; docs/DECISIONS.md:67-70, 74-76 | documented |
| MEDIA-25 | Tickets are stateless HMAC-SHA256 tokens (`payload.signature`, URL-safe base64, domain-separated) binding target/category, order (or lead), user, storage key, sha256, byte size, content type, filename and expiry. Any tampering → `BadSignature`; garbage → `Malformed`. | backend/src/media/upload_token.rs:1-4, 16-84, 121-137; docs/DECISIONS.md:72-73 | documented |
| MEDIA-26 | Tickets and the presigned PUT expire 2 hours after issue; completing an expired ticket → 422 `invalid_ticket` "upload ticket has expired; request a new upload". | backend/src/service/media.rs:26, 181-184, 214-219; backend/src/media/upload_token.rs:49-50, 80-82; docs/DECISIONS.md:73 | documented |
| MEDIA-27 | A ticket is not single-use: completing it again returns the existing record (idempotent) rather than an error. Only the user it was issued to may complete it (else 403). | backend/src/service/media.rs:220-222, 276-310; backend/src/api/media.rs:103-119 | implied |
| MEDIA-28 | Complete verifies the stored object before inserting: missing object → 422 `upload_missing`; size ≠ declared → 422 `upload_mismatch`; stored checksum ≠ declared hash → 422 `upload_mismatch`; if the store reports no checksum the object is re-read and re-hashed. | backend/src/service/media.rs:226-255; docs/DECISIONS.md:69-70 | documented |
| MEDIA-29 | Complete runs in one transaction that locks the order row; a newly created image writes an `image_add` audit entry and enqueues exactly one `process_image` job (dedupe key `process_image:{id}`). `immutable` is set to true only for `intake`. | backend/src/service/media.rs:262-311; backend/src/repo/images.rs:107-127 | documented |
| MEDIA-30 | Intake images are write-once: DB CHECK forces `immutable` for intake; a trigger refuses DELETE and any change to identity columns (`order_id`, `category`, `storage_key`, `content_hash`, `byte_size`, `uploaded_at`, `uploaded_by`, `immutable`, setting `deleted_at`) with 409 `immutable`; derived-copy columns stay writable. `DELETE /images/{id}` on an immutable image → 409 `immutable`. | backend/migrations/0004_media.sql:24-28, 35-66; backend/src/api/media.rs:217-233; backend/src/error.rs:139-143; docs/API.md:121 | documented |
| MEDIA-31 | Deleting a non-immutable image is a soft delete (`deleted_at`, `deleted_by`), requires `DeleteMedia` (Admin, Office), and writes an `image_delete` audit entry. | backend/src/api/media.rs:212-247; backend/src/repo/images.rs:181-190; backend/src/domain/role.rs:53-56 | documented |
| MEDIA-32 | `inspection` is a distinct image category travelling the same ticket → PUT → complete flow as other images. | backend/migrations/0025_image_category_inspection.sql:5-8; backend/src/domain/media.rs:15-18 | documented |

### Processing pipeline and reading media

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| MEDIA-33 | The original is stored byte-for-byte (EXIF included) and never modified; display (longest edge ≤ 2560 px, never upscaled) and thumbnail (≤ 480 px) copies are re-encoded JPEG quality 85, which strips all EXIF/GPS. | backend/src/media/pipeline.rs:1-3, 13-15, 113-149, 165-191; backend/migrations/0004_media.sql:7-11; docs/DECISIONS.md:65-67 | documented |
| MEDIA-34 | EXIF orientation (1-8) is applied to derived copies, and the stored width/height are after orientation. `captured_at` comes from EXIF `DateTimeOriginal` (fallback `DateTime`), using `OffsetTimeOriginal` when present, otherwise the business time zone (Europe/Budapest in tests); invalid dates yield null. | backend/src/media/pipeline.rs:27-29, 42-103, 201-232 | documented |
| MEDIA-35 | Decoding is limited to 20 000 × 20 000 px and 1 GiB allocation; undecodable input records a processing error on the image instead of failing the job. | backend/src/media/pipeline.rs:116-127, 193-199; backend/src/service/automation.rs:331-338 | documented |
| MEDIA-36 | `process_image` skips deleted or already-successfully-processed images; re-verifies the stored original's sha256 (mismatch → processing error "stored original does not match the recorded sha256"); HEIC/HEIF get processing error "previews for HEIC are not supported yet; original is stored" and no previews. Transient failures (storage) retry via the job queue with backoff up to `max_attempts`, then dead-letter. | backend/src/service/automation.rs:301-364; backend/src/jobs/mod.rs:94-110; backend/src/domain/media.rs:63-65; docs/DECISIONS.md:75-76, 79-80 | documented |
| MEDIA-37 | Image lists and inspection details expose presigned `thumb_url`/`display_url` valid 1 h, `null` until derived copies exist; the UI and emails use derived copies only. The original is served only via `GET /images/{id}/original` (presigned download, `Content-Disposition: attachment`, plus hex sha256), requires `ViewOriginalImages` (Admin, Office), and each access is logged. | backend/src/api/media.rs:27, 120-170, 174-209; backend/src/api/inspections.rs:238-241; docs/DECISIONS.md:66-67; docs/API.md:115-120 | documented |
| MEDIA-38 | Web inspection detail renders overview photos from `display_url`, falling back to `thumb_url`, using plain `<img>` (presigned URLs). (General order-level photo display on web is finding C1.) | frontend/src/components/inspections/InspectionSection.tsx:147, 176-179 | implied |
| MEDIA-39 | `GET /mobile/orders` (any authenticated user) returns at most 200 compact orders (id, number, title, partner, "make model", plate, stage key/label), open orders only unless `all=true`; `q` matches text and normalized plate; response carries `Cache-Control: private, max-age=60`. | backend/src/api/mobile.rs:1-2, 25-89; docs/API.md:125 | documented |

<a id="invoicing"></a>

## Invoicing & money

Scope: turning an order's line items into a NAV-reported invoice (számla), reversing it (storno), technically annulling a report, rendering proformas (díjbekérő), the MNB exchange rate on non-HUF documents, the minor-unit `Money` type and its rounding, the letters sent after each outcome, and reports that sum money. Entry points: the web order screen, tab "Számlák" (`frontend/src/app/[locale]/orders/[id]/page.tsx:222,526-529`), which holds `InvoicesSection` and `ProformasSection`. The API routes are `GET|POST /orders/{id}/invoices`, `GET /invoices/{id}`, `POST /invoices/{id}/storno`, `POST /invoices/{id}/annul`, `GET /invoices/{id}/chain` and `GET|POST /orders/{id}/proformas` (`backend/src/api/invoices.rs:27-35`). Background jobs `nav_submit_invoice` and `nav_annul_invoice` run in the worker (`backend/src/jobs/mod.rs:30-32,163-172`). The sidecar handles NAV traffic (`nav-sidecar/`). Android has no invoicing screen: the feature is web-only (`docs/integration-audit/01-system-map.md:26`). On Android, money shows up only as order/item values. Key files: `backend/src/domain/invoice.rs`, `backend/src/domain/money.rs`, `backend/src/service/invoicing.rs`, `backend/src/repo/invoices.rs`, `backend/src/repo/fx.rs`, `backend/src/integrations/nav.rs`, `backend/src/integrations/mnb.rs`, `backend/migrations/0006_reporting.sql`, `0020_document_kind_invoice.sql`, `0021_invoicing.sql`, `0022_invoice_email_templates.sql`, `nav-sidecar/src/{schemas,invoices}.ts`, `nav-sidecar/README.md`, `backend/tests/invoicing.rs`.

Note on terminology vs. the brief: the system has **no draft state and no corrective (MODIFY) invoice**. The lifecycle is `submitting → issued | rejected`, then `issued → stornoed` (via a storno document) or `issued|stornoed → annulled`. See Q-INV-1.

Law citations (2007. évi CXXVII. törvény, "Áfa tv.") come from general knowledge of the statute, not from the repo. Treat them as **assumed** until someone checks them against the current consolidated text.

### Configuration and permissions

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| INV-01 | When `NAV_SIDECAR_URL` is unset, every invoicing endpoint (issue, storno, annul, proforma, chain) refuses with rule error `invoicing_not_configured` and names `NAV_SIDECAR_URL` and the `NAV_SUPPLIER_*` variables. The rest of the CRM keeps working. | `backend/src/config.rs:137-141`, `backend/src/service/invoicing.rs:135-145`, `backend/tests/invoicing.rs:549-561` | documented |
| INV-02 | When `NAV_SIDECAR_URL` is set, startup requires every supplier identity field: `NAV_SUPPLIER_NAME`, `_TAX_NUMBER`, `_POSTAL_CODE`, `_CITY`, `_STREET_NAME`, `_STREET_CATEGORY`, `_STREET_NUMBER`. `NAV_SUPPLIER_BANK_ACCOUNT` is optional. A half-set identity must fail startup rather than produce invoices. | `backend/src/config.rs:226-260` | documented |
| INV-03 | Defaults: VAT rate 0.27 (`NAV_DEFAULT_VAT_RATE`), invoice prefix `AT`, proforma prefix `DB`, payment term 8 days (`NAV_PAYMENT_DAYS`), sidecar timeout 90 s per HTTP attempt. A default VAT rate below 0 or above 1 is a startup error. | `backend/src/config.rs:149-156,239-268` | documented |
| INV-04 | Issuing, stornoing and rendering a proforma need capability `IssueInvoices`, which only Admin and Office have. Designer and Viewer get 403. | `backend/src/domain/role.rs:37-39,54-56`, `backend/src/api/invoices.rs:104,152,231` | documented |
| INV-05 | Technical annulment needs `AnnulInvoices`, which only Admin has. | `backend/src/domain/role.rs:40-42,50-52`, `backend/src/api/invoices.rs:157,173` | documented |
| INV-06 | Any authenticated user can read the invoice and proforma lists, invoice detail and the NAV chain. None of these routes checks a capability. | `backend/src/api/invoices.rs:80-86,114-134,184-213` | implied |
| INV-07 | On the web, "Számla kiállítása", "Sztornó" and "Díjbekérő készítése" appear only for users who can edit orders (Admin/Office). "Technikai érvénytelenítés" appears only for admins. | `frontend/src/components/orders/InvoicesSection.tsx:69-70,132,246,251`, `frontend/src/components/orders/ProformasSection.tsx:35,70` | implied |

### Issuing an invoice (preconditions and snapshot)

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| INV-10 | `POST /orders/{id}/invoices` does not report synchronously. It answers **202** with the new invoice in status `submitting`, and a background job reports it. | `backend/src/api/invoices.rs:3-6,89-107`, `backend/src/service/invoicing.rs:4-9` | documented |
| INV-11 | An order with an invoice in `submitting` cannot get another one: 409 `invoice_in_flight`, "still being reported to NAV; wait for the verdict". | `backend/src/service/invoicing.rs:356-366`, `backend/tests/invoicing.rs:461-481` | documented |
| INV-12 | An order that already has a live invoice (kind `invoice`, status `issued`) cannot get another one: 409 `invoice_exists`. The live invoice must be stornoed first. A `rejected`, `stornoed` or `annulled` invoice does not block a new one. | `backend/src/service/invoicing.rs:327-331,367-377`, `backend/tests/invoicing.rs:483-497` | documented |
| INV-13 | The web disables "Számla kiállítása" while any invoice is `submitting` or a live invoice exists, with tooltip "Ehhez a megrendeléshez már tartozik élő számla. Előbb sztornózza." / "Bejelentés a NAV felé folyamatban…". | `frontend/src/components/orders/InvoicesSection.tsx:86-87,136-137`, `frontend/src/messages/hu.json:938-939` | implied |
| INV-14 | An order with no line items cannot be invoiced: rule error `no_items`, "an invoice needs at least one line item on the order". | `backend/src/service/invoicing.rs:381-387`, `backend/tests/invoicing.rs:501-516` | documented |
| INV-15 | The customer must be invoiceable before a number is drawn. With no postal code, city or street address, the request fails with `invoice_data_missing`, and the message names the partner and the missing field. A street address that `split_address_line` cannot split into name, category and house number is refused with the same code. The system never guesses an address. | `backend/src/service/invoicing.rs:164-198,389-394`, `backend/src/domain/invoice.rs:94-100`, `backend/tests/invoicing.rs:520-545` | documented |
| INV-16 | Customer VAT status: a HU business must have a tax number (else `invoice_data_missing`) and goes to NAV as `DOMESTIC`. A HU private person goes without a tax number as `PRIVATE_PERSON`. Any non-HU partner goes as `OTHER` with no Hungarian tax number, and its EU VAT number is not sent in the HU field. | `backend/src/service/invoicing.rs:200-215` | documented |
| INV-17 | Address splitting: the **last** category word in the line wins, longest categories match first ("körút" before "út"), and the inflections `tere→tér`, `útja→út`, `u.→utca`, `krt.→körút` are normalised. Text with no street name, no house number or no category (e.g. "hrsz 0123/4") is refused (`None`). | `backend/src/domain/invoice.rs:69-160,236-252` | documented |
| INV-18 | A non-HUF invoice needs a stored MNB rate for its currency on or before the issue date. Without one the request fails with `fx_rate_missing` ("fetch the rates and try again"), and no number is drawn. | `backend/src/service/invoicing.rs:303-325,395` | documented |
| INV-19 | Issuing takes a **snapshot**: each order item becomes an `invoice_lines` row (position 1..n, description, quantity, unit `PIECE`, net unit price, VAT rate, net amount, VAT amount). Later edits to the order never change an issued invoice, and the NAV report is built from the snapshot, not the order. | `backend/migrations/0021_invoicing.sql:9-13`, `backend/src/service/invoicing.rs:252-301,1215-1218,1236-1260` | documented |
| INV-20 | `order_items.unit_price` is treated as a **net** unit price in the order's currency. Line items carry no VAT. VAT is added only when the invoice is made, and one rate covers every line of the invoice. | `docs/DECISIONS.md:22-23`, `backend/migrations/0021_invoicing.sql:12-13`, `backend/src/service/invoicing.rs:264,388` | documented |
| INV-21 | Invoice currency equals order currency (HUF or EUR only). Because of the composite FK, line items always share the order's currency. | `backend/migrations/0003_orders.sql:28,44-45,64-66`, `backend/migrations/0021_invoicing.sql:35`, `backend/src/service/invoicing.rs:354` | documented |
| INV-22 | Header totals: `net_amount` = Σ line net, `vat_amount` = Σ line VAT, `gross_amount` = net + VAT, all in minor units. Example: 1 × 1 000 000.00 HUF at 0.27 gives net 100 000 000, VAT 27 000 000, gross 127 000 000 minor units. | `backend/src/service/invoicing.rs:259-271,409-411`, `backend/tests/invoicing.rs:76-88` | documented |
| INV-23 | The invoice row, its lines, the audit entry `invoice_issue` (invoice_id, number, gross, currency) and the submit job are committed in **one** transaction. If any step fails, none of them persists. | `backend/src/service/invoicing.rs:350-435` | implied |
| INV-24 | The submit job's dedupe key is `nav_submit:{invoice_id}`: one submission job per invoice, ever. | `backend/src/service/invoicing.rs:463-483` | documented |

### Dates and payment terms

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| INV-30 | Issue date defaults to "today" in the business timezone (`BUSINESS_TIMEZONE`, default Europe/Budapest), not UTC. | `backend/src/service/invoicing.rs:71,343-344`, `backend/src/service/mod.rs:16-18`, `backend/src/config.rs:458` | documented |
| INV-31 | Fulfilment/delivery date (teljesítés dátuma) defaults to the issue date. It is stored on the invoice (`delivery_date NOT NULL`) and sent to NAV. | `backend/src/service/invoicing.rs:73-74,345`, `backend/migrations/0021_invoicing.sql:37`, `backend/src/service/invoicing.rs:1242` | documented |
| INV-32 | Payment due date defaults to issue date + `NAV_PAYMENT_DAYS` (default 8) calendar days. An invoice always carries a payment date when it is created. | `backend/src/service/invoicing.rs:75-76,346-348,408`, `backend/src/config.rs:268` | documented |
| INV-33 | The web issue dialog offers only VAT rate and payment date. Both may be left empty, and the server then fills them. Issue date and delivery date cannot be set from the UI. | `frontend/src/components/orders/InvoicesSection.tsx:340-352,401-405` | implied |
| INV-34 | Payment method is one of `TRANSFER` (default), `CASH`, `CARD`, `VOUCHER`, `OTHER`. The sidecar refuses any other value (strict enum). It goes on the submit job, not on the invoice row. | `backend/src/service/invoicing.rs:77-78,1246`, `nav-sidecar/src/schemas.ts:120` | documented |
| INV-35 | Letters print dates as `YYYY.MM.DD.` (Hungarian style, e.g. `2026.09.21.`). | `backend/src/service/invoicing.rs:1078-1080` | documented |
| INV-36 | Every web date is rendered through `<DateDisplay>`. | `docs/history/FRONTEND_PLAN.md:81` (M3), `frontend/src/components/orders/InvoicesSection.tsx:229` | documented |
| INV-37 | (Law) The invoice must show issue date, a sequential number, supplier and customer names and addresses, supplier tax number, customer tax number where required, description, quantity, unit, net unit price, tax base, VAT rate, VAT amount and the fulfilment date when it differs from the issue date. | Áfa tv. §169 a)–m) | assumed |
| INV-38 | (Law) The invoice must be issued no later than 15 days after the fulfilment date. | Áfa tv. §163(2) | assumed |

### VAT rate and rounding

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| MONEY-01 | VAT rates are **fractions**, never percentages: 0.27 means 27%. The API refuses a rate below 0 or above 1 with a validation error ("0.27 for 27%"). Rate 0 is allowed. | `backend/src/domain/invoice.rs:194-202,274-280`, `backend/src/service/invoicing.rs:67-70,339-342` | documented |
| MONEY-02 | Allowed VAT rates are also enforced at the DB (`vat_rate NUMERIC(5,4) CHECK 0..1`, so 0.2700 is stored for 27%) and at the sidecar (`vatPercentage` 0..1, else 400). | `backend/migrations/0021_invoicing.sql:93-94`, `nav-sidecar/src/schemas.ts:90-97`, `nav-sidecar/README.md:161-162`, `backend/tests/invoicing.rs:87` | documented |
| MONEY-03 | The web VAT field says "Tizedes tört, nem százalék: 27% helyett 0.27. Üresen hagyva az alapértelmezett." (A decimal fraction, not a percentage: 0.27 instead of 27%. Left empty, the default applies.) | `frontend/src/messages/hu.json:944-945`, `frontend/src/components/orders/InvoicesSection.tsx:367-380` | documented |
| MONEY-04 | Line net = round(unit_price_minor × quantity) to whole minor units, **half away from zero**. Examples: 1 EUR-cent × 0.5 = 1; −1 × 0.5 = −1; 333 × 1.5 = 500; 1999 × 2.125 = 4248. | `backend/src/domain/money.rs:117-126,219-248`, `backend/migrations/0003_orders.sql:59`, `docs/DECISIONS.md:36-38` | documented |
| MONEY-05 | VAT is computed **per line**: line VAT = round(line net minor × rate), half away from zero. Invoice VAT = Σ line VAT, not VAT on the net total. Example: 2.5 × 4000.00 HUF at 0.27 gives net 1 000 000 and VAT 270 000 minor units. | `backend/src/domain/invoice.rs:171-186,254-272`, `backend/src/service/invoicing.rs:264-271` | documented |
| MONEY-06 | A discount line (negative unit price) stays negative in both net and VAT (−1000 net → −270 VAT at 27%). | `backend/src/domain/invoice.rs:267-271`, `backend/migrations/0021_invoicing.sql:91`, `backend/migrations/0003_orders.sql:58` | documented |
| MONEY-07 | Rust `Money` rounding and Postgres `round()` use the same rule, so an invoice total and the order total never differ for rounding reasons. The order detail endpoint cross-checks both and logs an error if they disagree. | `backend/src/domain/invoice.rs:173-175`, `docs/DECISIONS.md:36-38` | documented |
| MONEY-08 | Quantity is `NUMERIC(12,3)` and must be > 0, on both order items and invoice lines. | `backend/migrations/0003_orders.sql:57`, `backend/migrations/0021_invoicing.sql:88` | documented |
| MONEY-09 | Amounts go to the sidecar as **decimal strings** with the currency exponent (139 700 minor → `"1397.00"`). Quantities and VAT rates also go as normalised decimal strings (`"0.27"`, not `0.27`). Money never passes through a float. | `backend/src/domain/invoice.rs:162-169,282-292`, `backend/src/integrations/nav.rs:54-61`, `nav-sidecar/README.md:163-164` | documented |
| MONEY-10 | Unit of measure is always `PIECE` on invoices built from orders. The sidecar accepts only NAV's list (PIECE, KILOGRAM, TON, KWH, DAY, HOUR, MINUTE, MONTH, LITER, KILOMETER, CUBIC_METER, METER, LINEAR_METER, CARTON, PACK, OWN). | `backend/src/service/invoicing.rs:279,288`, `backend/migrations/0021_invoicing.sql:89-90`, `nav-sidecar/src/schemas.ts:28-45` | documented |
| MONEY-11 | Prices are reported to NAV in **net** mode (the backend never sends `priceMode: gross`). | `backend/src/integrations/nav.rs:66-86`, `nav-sidecar/README.md:170-172` | implied |
| MONEY-12 | A NAV document holds at most 100 lines (sidecar schema `lines.min(1).max(100)`). | `nav-sidecar/src/schemas.ts:128` | documented |

### Money type, currencies and display

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| MONEY-20 | All amounts are integer **minor units** (fillér / eurocent) with an explicit currency, and only `HUF` and `EUR` exist. Both use exponent 2, so HUF is stored in fillér. | `backend/src/domain/money.rs:1-4,14-33`, `docs/API.md:3-4`, `backend/migrations/0021_invoicing.sql:15-16` | documented |
| MONEY-21 | Adding or subtracting two amounts in different currencies is an error (`CurrencyMismatch`), never a number. `Money` has no `+` operator, and overflow is an error. The sum of an empty set is zero. | `backend/src/domain/money.rs:1-4,89-115,192-216` | documented |
| MONEY-22 | Currency conversion: `minor × rate` rounded half away from zero to the target's minor units. Converting to the same currency ignores the rate. Examples: 10.00 EUR @ 365.22 = 3652.20 HUF; 0.01 EUR @ 365.225 → 365 fillér; @ 365.5 → 366. | `backend/src/domain/money.rs:128-141,252-272` | documented |
| MONEY-23 | Web and Android display amounts without float arithmetic and without silently rounding fillér. A HUF amount with zero fillér shows as `1 234 Ft`; nonzero fillér shows as `1 234,50 Ft`. EUR always shows two decimals (`1 234,50 €`). (Cross-client divergence is already reported as A1/A2 and not repeated here.) | `docs/history/FRONTEND_PLAN.md:65,164,182-183`, `frontend/src/lib/utils/format.ts:8-30`, `android/app/src/main/java/hu/autotherm/autocrm/util/Format.kt:19-34`, `docs/history/REMEDIATION.md:16` | documented |
| MONEY-24 | Clients never compute totals, VAT or conversions. Line totals, order totals and HUF normalisation come from the backend. | `docs/history/FRONTEND_PLAN.md:65,187`, `docs/history/AUDIT.md:716` | documented |
| MONEY-25 | The invoice and proforma lists show each document's **gross** amount through `<Money>`. | `frontend/src/components/orders/InvoicesSection.tsx:230`, `frontend/src/components/orders/ProformasSection.tsx:103` | documented |
| MONEY-26 | Letters render totals in the machine-style `Money` Display form, e.g. `1270000.00 HUF`, with a `-` sign on a storno. | `backend/src/service/invoicing.rs:1073-1076,1109-1112`, `backend/src/domain/money.rs:152-166,274-277` | implied |

### FX: MNB rate on non-HUF documents

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| INV-40 | The rate source is the official MNB rate (SOAP `GetExchangeRates`), stored as HUF per 1 unit of the base currency and divided by MNB's `unit` (JPY per 100). Hungarian decimal commas are parsed. Rates ≤ 0 are refused. | `backend/src/integrations/mnb.rs:1-5,100-150,173-189`, `backend/migrations/0006_reporting.sql:4-13` | documented |
| INV-41 | Rates are fetched daily after 12:30 Budapest time. Re-fetching a day overwrites it only if MNB changed the value. | `README.md:59`, `backend/src/repo/fx.rs:16-38` | documented |
| INV-42 | A non-HUF invoice or proforma uses the latest stored MNB rate **on or before the issue date**, so a weekend or holiday issue picks up the last working day's rate. HUF documents send no exchange rate. | `backend/src/repo/fx.rs:74-94`, `backend/src/service/invoicing.rs:303-325,987,1237` | documented |
| INV-43 | The rate is looked up again at submit time from the invoice's stored issue date. It is not stored on the invoice row. | `backend/src/service/invoicing.rs:1237`, `backend/migrations/0021_invoicing.sql:25-73` | implied |
| INV-44 | (Law) For a foreign-currency invoice, the VAT amount must also be stated in HUF, converted at the rate chosen under Áfa tv. §80. That rate applies at the date when the tax becomes payable, which is normally the fulfilment date (§55, §60), not the issue date. | Áfa tv. §80(1)-(2), §172 (VAT in HUF on the invoice) | assumed |
| INV-45 | NAV requires the exchange rate to HUF on any non-HUF invoice, and the sidecar refuses a non-HUF invoice without one. | `backend/src/integrations/nav.rs:76-78`, `nav-sidecar/src/schemas.ts:115-118`, `nav-sidecar/README.md:173` | documented |

### Numbering

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| INV-50 | Invoice numbers have the form `{prefix}{year}-{seq:04}`, e.g. `AT2026-0001`. The year is the **issue date's** year, and the sequence restarts at 1 for each prefix+year series. | `backend/src/domain/invoice.rs:188-192,294-298`, `backend/src/repo/invoices.rs:133-150`, `backend/src/service/invoicing.rs:397` | documented |
| INV-51 | Next number = max existing sequence in that series + 1. It is drawn under a transaction-scoped advisory lock (`autocrm.invoice_number`), so two concurrent issues never draw the same number. | `backend/src/repo/invoices.rs:130-150` | documented |
| INV-52 | An invoice number is unique forever and never reused, even after a rejection. A rejected number is spent, and a corrected invoice is a new document with a new number. | `backend/migrations/0021_invoicing.sql:28-30`, `backend/src/domain/invoice.rs:27-28`, `backend/src/repo/invoices.rs:325-326` | documented |
| INV-53 | A storno draws its **own** number from the same invoice series (not the original's number with a suffix). | `backend/src/service/invoicing.rs:518`, `backend/tests/invoicing.rs:214` | documented |
| INV-54 | Proformas use a separate series and lock (`autocrm.proforma_number`, prefix `DB`, e.g. `DB2026-0001`) and never consume an invoice number. | `backend/src/repo/invoices.rs:152-175`, `backend/tests/invoicing.rs:370-374` | documented |
| INV-55 | A proforma number is drawn and committed **before** rendering. A failed render may leave a gap in the proforma series, and that is accepted. | `backend/src/service/invoicing.rs:989-995` | documented |
| INV-56 | (Law) Invoice numbers must be sequential and uniquely identify the invoice. | Áfa tv. §169 b) | assumed |

### NAV reporting (submit job) and statuses

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| INV-60 | Status enum: `submitting`, `issued`, `rejected`, `stornoed`, `annulled`. Kind enum: `invoice`, `storno`. Only `issued` counts as "live". | `backend/migrations/0021_invoicing.sql:18-23`, `backend/src/domain/invoice.rs:14-57` | documented |
| INV-61 | Every new invoice or storno row starts as `submitting`, with `submitted_at = now()`. | `backend/src/repo/invoices.rs:177-205` | documented |
| INV-62 | The job acts only on a row still in `submitting`. For a row in any other state it does nothing and succeeds (a retry of a finished job, or a human intervened). A row that has vanished is logged and skipped. | `backend/src/service/invoicing.rs:666-675` | documented |
| INV-63 | When NAV stores the report (`DONE`), the row becomes `issued` with `nav_transaction_id`, `nav_status`, `nav_messages` (warnings included) and `issued_at` set, and `nav_error_code` and `nav_message` cleared. | `backend/src/repo/invoices.rs:293-323`, `backend/src/service/invoicing.rs:767-786`, `backend/tests/invoicing.rs:102-108` | documented |
| INV-64 | When NAV or the sidecar refuses with a non-retryable fault, the row becomes `rejected` with `nav_status = 'ABORTED'`, NAV's own fault code kept verbatim in `nav_error_code` (e.g. `INVOICE_NUMBER_ALREADY_EXISTS`), the message, all messages, and the transaction id when one exists. An audit entry `invoice_rejected` is written, and the job **succeeds**: it is not retried. | `backend/src/service/invoicing.rs:660-665,714-748`, `backend/src/repo/invoices.rs:325-357`, `backend/migrations/0021_invoicing.sql:43-46`, `backend/tests/invoicing.rs:124-165` | documented |
| INV-65 | Retryable failures are sidecar unreachable/timeout, a non-envelope 5xx, and sidecar `kind: nav_unreachable` (504, the verdict never settled). The row **stays `submitting`**, `nav_message` records the attempt, and the job fails so the queue retries it with exponential backoff (30 s, 1 m, 2 m, … capped at 6 h). A job has at most 5 attempts before it is dead-lettered. | `backend/src/integrations/nav.rs:232-259,426-446`, `backend/src/service/invoicing.rs:708-713,752-765`, `backend/src/repo/jobs.rs:185-202`, `backend/migrations/0001_foundation.sql:92-96` | documented |
| INV-66 | A sidecar reply that breaks the contract (non-JSON 4xx, undecodable body) is `Contract` and is **not** retried. For a submit, the row is then marked `rejected`. | `backend/src/integrations/nav.rs:241-243,257,422-443`, `backend/src/service/invoicing.rs:714-725` | implied |
| INV-67 | A timeout is **not** proof that nothing was reported. The sidecar may have submitted, so the backend must treat the result as "unknown" and must never mark the row rejected on a timeout. | `backend/src/integrations/nav.rs:409-413`, `backend/src/service/invoicing.rs:709-710` | documented |
| INV-68 | Sidecar error kinds and HTTP codes: `bad_request` 400, `validation` 422 (caught locally, nothing sent), `nav_rejected` 422 (NAV has a transaction, `ABORTED`), `not_found` 404, `nav_error` 502, `nav_unreachable` 504, `config` 500, `internal` 500. | `nav-sidecar/README.md:392-407`, `backend/src/integrations/nav.rs:209-225` | documented |
| INV-69 | The sidecar refuses to report for any supplier other than the taxpayer it authenticates as (`NAV_TAX_NUMBER`) and checks this before anything is sent. | `nav-sidecar/README.md:165-167`, `nav-sidecar/src/invoices.ts:123-136` | documented |
| INV-70 | The sidecar submits with `manageInvoice` (CREATE) and polls until NAV gives a verdict (`RECEIVED → PROCESSING → DONE/ABORTED`), within `NAV_POLL_TIMEOUT_MS` (default 60 s). The backend timeout (90 s) must stay above that polling time. | `nav-sidecar/README.md:7-11,89-91,184-185`, `nav-sidecar/src/invoices.ts:163-200`, `backend/src/config.rs:145-147` | documented |
| INV-71 | The web polls the order's invoice list every 3 s while any invoice is `submitting`, and stops once none is. The row shows "Bejelentés a NAV felé folyamatban…" (reporting to NAV in progress) while in flight. | `frontend/src/components/orders/InvoicesSection.tsx:26-27,77-83,239` | documented |
| INV-72 | A rejected row shows "A NAV elutasította — {nav_error_code}" plus NAV's message. It is never flattened into a generic "hiba" (error). Messages at level ERROR and WARN appear under every row, with code and path. | `frontend/src/components/orders/InvoicesSection.tsx:5-8,52-55,258-277`, `frontend/src/messages/hu.json:941` | documented |
| INV-73 | Status labels: submitting "Bejelentés folyamatban", issued "Kiállítva", rejected "Elutasítva", stornoed "Sztornózva", annulled "Érvénytelenítve". A storno also carries a "Sztornó" badge. | `frontend/src/messages/hu.json:943,965-971`, `frontend/src/components/orders/InvoicesSection.tsx:223-228` | documented |
| INV-74 | The payload sent to NAV includes the order number (`orderNumbers: [order.number]`), the supplier from config (country HU), the customer from the partner record at **submit time**, and the snapshot lines. | `backend/src/service/invoicing.rs:1219-1261` | documented |

### PDF and letters after reporting

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| INV-80 | Once NAV has stored the report, the invoice PDF is fetched from the sidecar. The sidecar renders it from **what NAV holds**, not from a local copy. The PDF is filed as an order document of kind `invoice`, named `{number}.pdf`, and linked in `invoices.document_id`. | `backend/src/service/invoicing.rs:813-831`, `nav-sidecar/README.md:288-299`, `backend/migrations/0020_document_kind_invoice.sql:1-6`, `backend/tests/invoicing.rs:409-447` | documented |
| INV-81 | If the PDF cannot be stored, the invoice must **still** be `issued`. The missing PDF shows on the row and can be fetched again. | `backend/src/service/invoicing.rs:788-797` | documented |
| INV-82 | When `send_email` is true (the default), a letter goes to the customer after NAV stores the report: template `invoice_issued` for an invoice, `invoice_stornoed` for a storno, `invoice_annulled` for an annulment. The PDF is attached except on the annulment letter, which carries no document. With no customer e-mail address, no letter is sent and a warning is logged. | `backend/src/service/invoicing.rs:79-81,799-803,1082-1155`, `backend/migrations/0022_invoice_email_templates.sql:126-170`, `backend/tests/invoicing.rs:110-120,450-457` | documented |
| INV-83 | At most one letter goes out per invoice per outcome, whatever the retries: idempotency key `{trigger}:{invoice_id}`. | `backend/src/service/invoicing.rs:1146-1147` | documented |
| INV-84 | Failing to queue a letter never undoes or fails the report. It is logged only. | `backend/src/service/invoicing.rs:799-803,939-943` | documented |
| INV-85 | The invoice letter states number, issue date, payment due date and total, and says the data was sent to NAV Online Számla. The storno letter names both the storno number and the original number and asks the customer to treat the original as invalid. | `backend/migrations/0022_invoice_email_templates.sql:127-157` | documented |
| INV-86 | Template wording is editable afterwards in settings. The migration only seeds it (`ON CONFLICT DO NOTHING`). | `backend/migrations/0022_invoice_email_templates.sql:120-121,186` | documented |

### Storno (cancellation)

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| INV-90 | Only an `invoice`-kind document in status `issued` can be stornoed. Stornoing a storno, or an invoice that is submitting, rejected, stornoed or annulled, fails with 422 `not_stornoable`. | `backend/src/service/invoicing.rs:500-515`, `backend/src/api/invoices.rs:143` | documented |
| INV-91 | A storno is a **new document** (kind `storno`) with its own number, `original_invoice_id` pointing at the original (DB check: set exactly on a storno, never self), the original's currency, delivery date and payment date, and an issue date defaulting to today in the business timezone. | `backend/src/service/invoicing.rs:485-539`, `backend/migrations/0021_invoicing.sql:33-34,69-72` | documented |
| INV-92 | A storno's header and line amounts are the original's **negated** (net, VAT, gross, unit price). Quantity and VAT rate stay as they were, and no flag is needed to read it as a reversal. | `backend/src/service/invoicing.rs:530-556`, `backend/migrations/0021_invoicing.sql:16`, `backend/tests/invoicing.rs:213` | documented |
| INV-93 | At most one non-rejected storno can exist per original (partial unique index). A rejected storno attempt does not count, and the next attempt gets a new number. | `backend/migrations/0021_invoicing.sql:75-79` | documented |
| INV-94 | The original stays `issued` until NAV stores the storno. Only then, in the same transaction that marks the storno `issued`, does the original become `stornoed` (and only if it was still `issued`). | `backend/src/service/invoicing.rs:783-786`, `backend/src/repo/invoices.rs:359-367`, `backend/tests/invoicing.rs:228-236` | documented |
| INV-95 | The storno goes to NAV as a STORNO operation that names the original's number in the path and the storno's number and issue date in the body. The sidecar reads the original back from NAV and reverses what NAV holds. | `backend/src/service/invoicing.rs:684-700`, `nav-sidecar/src/invoices.ts:228-259`, `nav-sidecar/README.md:206-232` | documented |
| INV-96 | The storno is written in one transaction together with the audit entry `invoice_storno` (storno id and number, original id and number) and its submit job. | `backend/src/service/invoicing.rs:558-573` | documented |
| INV-97 | Web confirmation text: stornoing cannot be undone, and after a mistake a new invoice must be issued. | `frontend/src/messages/hu.json:952-953`, `frontend/src/components/orders/InvoicesSection.tsx:178-186` | documented |
| INV-98 | (Law) A document that modifies or cancels an invoice must reference the original invoice's number unambiguously. | Áfa tv. §170(1) | assumed |

### Technical annulment

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| INV-100 | Technical annulment says the **report** should never have been filed, which is different from a storno (the invoice was wrong). Only invoices in status `issued` or `stornoed` can be annulled. Otherwise the request fails with `not_annullable`. | `backend/src/service/invoicing.rs:49-56,579-612`, `frontend/src/messages/hu.json:956` | documented |
| INV-101 | The code must be one of `ERRATIC_DATA`, `ERRATIC_INVOICE_NUMBER`, `ERRATIC_INVOICE_ISSUE_DATE`, `ERRATIC_ELECTRONIC_HASH_VALUE`, else a validation error. A reason is required (the web disables submit while it is blank, and the sidecar caps it at 1024 characters). | `backend/src/service/invoicing.rs:51-56,588-594`, `nav-sidecar/src/schemas.ts:161-172`, `frontend/src/components/orders/InvoicesSection.tsx:480-483`, `backend/tests/invoicing.rs:313-339` | documented |
| INV-102 | The request answers 202 and queues job `nav_annul_invoice` (dedupe `nav_annul:{id}`), with audit entry `invoice_annul`. The status does not change until NAV accepts. | `backend/src/service/invoicing.rs:614-637`, `backend/src/api/invoices.rs:157-176` | documented |
| INV-103 | When NAV accepts, the row becomes `annulled` with `annulment_transaction_id`, code, reason and `annulled_at`, an `invoice_annulled` audit entry is written, and the annulment letter (no attachment) is queued. NAV's acceptance is not final: a person must still approve it in the Online Számla portal. | `backend/src/service/invoicing.rs:913-944`, `backend/src/repo/invoices.rs:369-395`, `nav-sidecar/README.md:247-249`, `backend/tests/invoicing.rs:298-306` | documented |
| INV-104 | When NAV refuses the annulment, the invoice **keeps its status**, and the fault code and message are recorded on the row. Retryable failures are retried like submissions. | `backend/src/service/invoicing.rs:893-910` | documented |
| INV-105 | Re-running the annul job on a row that is already `annulled` does nothing. | `backend/src/service/invoicing.rs:878-880` | documented |

### Proformas (díjbekérő)

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| INV-110 | A proforma is **not** a tax document. It is never reported to NAV and has no status, transaction or chain. The sidecar answers `reportedToNav: false` and makes no NAV call on `/proformas`. | `backend/migrations/0021_invoicing.sql:101-104`, `backend/src/service/invoicing.rs:949-953`, `nav-sidecar/README.md:28-32,301-308`, `backend/src/integrations/nav.rs:165-166` | documented |
| INV-111 | A proforma can be made at any time, before or without an invoice and whatever the invoice state. It still needs at least one line item (`no_items`) and an invoiceable customer. | `backend/src/api/invoices.rs:215-218`, `backend/src/service/invoicing.rs:976-982,1008`, `frontend/src/messages/hu.json:979` | documented |
| INV-112 | Proforma creation is synchronous: **201** with the proforma and the queued `email_id` (null when no letter was sent). The PDF is stored as a document of kind `proforma` (always present) and a row is inserted in `proformas`. | `backend/src/api/invoices.rs:219-238`, `backend/src/service/invoicing.rs:1015-1068`, `backend/tests/invoicing.rs:376-405` | documented |
| INV-113 | Proforma amounts use the same snapshot, rounding and VAT rate rules as invoices. Its dates are issue date (default today), payment date (default issue + payment days), delivery date = issue date, and payment method is always TRANSFER. | `backend/src/service/invoicing.rs:960-1013` | documented |
| INV-114 | The web proforma dialog offers a payment date and a note, and says the proforma will be e-mailed to the customer and nothing is reported to NAV. The letter states that a díjbekérő is a payment request, not a tax document, and gives no VAT deduction right. | `frontend/src/components/orders/ProformasSection.tsx:109-146`, `frontend/src/messages/hu.json:977-982`, `backend/migrations/0022_invoice_email_templates.sql:122-125,172-185` | documented |
| INV-115 | Proformas and invoices appear in separate sections and are never mixed in one list. | `frontend/src/components/orders/InvoicesSection.tsx:10-11`, `frontend/src/components/orders/ProformasSection.tsx:5-9`, `frontend/src/app/[locale]/orders/[id]/page.tsx:526` | documented |
| INV-116 | Proforma letter idempotency key: `proforma:{id}`. The PDF is always attached. | `backend/src/service/invoicing.rs:1157-1201` | documented |

### Reading, chain and reports

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| INV-120 | `GET /orders/{id}/invoices` lists invoices newest first (`id DESC`). `GET /invoices/{id}` returns the invoice with its lines (by position) and the stored PDF's filename. | `backend/src/repo/invoices.rs:265-291`, `backend/src/api/invoices.rs:64-134` | documented |
| INV-121 | The chain comes live from NAV (`queryInvoiceChainDigest`, every page walked) as steps `{invoice_number, operation: CREATE\|MODIFY\|STORNO, ins_date (UTC), original_invoice_number}`. A NAV failure reaches the caller with its code (`nav_rejected`, `nav_unreachable`, or 404). The web hides the chain toggle while `submitting`. | `backend/src/api/invoices.rs:37-62,178-198`, `backend/src/service/invoicing.rs:1205-1213,1264-1288`, `nav-sidecar/src/invoices.ts:336-389`, `frontend/src/components/orders/InvoicesSection.tsx:241-245` | documented |
| INV-122 | Revenue reports sum **order values** (Σ round(quantity × unit_price) of order items), not invoices. They split HUF and EUR totals, give a HUF-normalised total at the order's `valuation_date` MNB rate (latest rate within the 10 days before that date), and count orders with no usable rate as `missing_fx` instead of using today's rate. | `backend/migrations/0006_reporting.sql:38-69`, `backend/src/repo/reports.rs:31-34`, `docs/DECISIONS.md:25-28,106-108` | documented |
| INV-123 | An order's valuation date defaults to its creation day and is never "today at report time", so past figures do not move. | `backend/migrations/0003_orders.sql:29-31`, `docs/DECISIONS.md:25-28` | documented |

<a id="email"></a>

## Email & communications

Scope: every outbound letter AutoCRM sends: manual compose, the lead quotation letter, newsletter blasts, and automatic mail (blocker nudges, stage-change and ready-for-pickup notices, stalled-order alerts, invoice/storno/annulment/proforma letters). Also covered: templates and placeholder rendering, the suppression list, newsletter subscriptions and unsubscribe, the email transport settings (migration 0007), and the Postgres job queue that delivers mail. Entry points: web `/emails` (inbox), `/emails/[id]` (detail), `/emails/new` (ComposeForm), QuotationDialog on a lead, NewsletterList, and the public `/newsletter/unsubscribe` page. Android: `ui/emails/EmailScreens.kt` and `EmailComposeScreen.kt`. API: `POST /emails`, `POST /emails/preview`, `GET /emails`, `GET /emails/{id}`, `POST /emails/{id}/cancel|retry`, `/email-templates*`, `/email-suppressions*`, `/newsletter/*`, `POST /leads/{id}/quotation`, `PUT /settings`, `/admin/jobs*`, `/admin/email/test`, `/admin/run/{kind}`. Key backend files: `backend/src/service/email.rs`, `domain/email.rs`, `domain/template.rs`, `integrations/email.rs`, `repo/emails.rs`, `repo/templates.rs`, `repo/newsletter.rs`, `api/email.rs`, `api/newsletter.rs`, `jobs/mod.rs`, `repo/jobs.rs`, `service/automation.rs`, and migrations 0005, 0007, 0016, 0022, 0024. Web and Android email viewing gaps are already reported as C7 in `docs/integration-audit/02-findings.md:153`; this spec does not repeat them.

### Email log, statuses and the one pipeline

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| MAIL-01 | Every email, whether manual, quotation, newsletter or automatic, is inserted as an `email_messages` row first. A `send_email` job keyed `send_email:{id}` then delivers it. No code path sends customer mail directly, except the admin test mail. | backend/src/service/email.rs:1-2, 606-613, 774-781, 933-940, 1031-1038; backend/src/api/admin.rs:231-235 | documented |
| MAIL-02 | Email rows are never deleted. Cancelling sets `status='cancelled'`, `cancelled_at` and `cancelled_by`; it does not remove the row. | backend/migrations/0005_email.sql:72; backend/src/repo/emails.rs:1, 274-290 | documented |
| MAIL-03 | Status is one of `queued`, `sending`, `sent`, `failed`, `cancelled`, `needs_review`. A new row starts as `queued`, with `attempts=0` and `send_after=now()` unless a later time is given. | backend/migrations/0005_email.sql:70, 99-104; backend/src/repo/emails.rs:93 | documented |
| MAIL-04 | An email is about at most one of order, lead or partner. A `blocker_id` requires an `order_id`. `is_automatic` is true exactly when `sent_by` is NULL. Manual compose with more than one of order/lead/partner is rejected with 422 "an email is about at most one of order, lead or partner". | backend/migrations/0005_email.sql:110-112; backend/src/service/email.rs:442-451; backend/src/repo/emails.rs:102 | documented |
| MAIL-05 | For automatic mail, if `order_id` is set, the lead and partner are dropped. If a lead is set, the partner is dropped. The row therefore always meets MAIL-04. | backend/src/service/email.rs:1000-1011 | documented |
| MAIL-06 | A blocker nudge carries both `blocker_id` and its `order_id`, so it appears in the order's correspondence history. | backend/migrations/0005_email.sql:75-76; backend/src/service/automation.rs:78-82 | documented |
| MAIL-07 | `GET /emails` returns summaries without bodies, newest first (`queued_at DESC, id DESC`). It filters by order, lead, partner, status, `attention` (only `failed` and `needs_review`) and `q`. The `partner_id` filter also includes mail about that partner's orders and leads. `q` is a case-insensitive search over subject and recipient across the whole log, not just the loaded page. Any authenticated user may list and read emails. | backend/src/api/email.rs:38-82, 84-99; backend/src/repo/emails.rs:44-45, 146-193 | documented |
| MAIL-08 | `GET /admin/status` reports `emails_needing_attention` = the count of `failed` + `needs_review` rows. It also reports the effective email mode, SMTP host, active redirect and the kill switch. | backend/src/api/admin.rs:79-115; backend/src/repo/emails.rs:292-298; docs/email-google-workspace.md:113-114 | documented |

### Manual compose, preview and quotation

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| MAIL-10 | Only users with `SendEmail` (Admin, Office) may preview or send manual mail. Designer and Viewer get 403. | backend/src/api/email.rs:111, 125; backend/src/domain/role.rs:54-56, 99 | documented |
| MAIL-11 | The recipient and each CC are trimmed and lowercased. The request is rejected (422) when an address is longer than 254 characters, contains whitespace, control characters or `, ; < > "`, has no `@`, has an empty local part, or its domain is shorter than 3 characters, has no dot, or starts or ends with a dot. This blocks header and recipient injection. | backend/src/domain/email.rs:117-140, 267-276; backend/src/service/email.rs:452-461 | documented |
| MAIL-12 | Subject and body come from the request when given, otherwise from `template_key`. An unknown `template_key` returns 404 "email template". A blank subject gives 422 "subject is required"; a blank body gives 422 "body is required". | backend/src/service/email.rs:463-482 | documented |
| MAIL-13 | Preview (`POST /emails/preview`) renders exactly what send would produce: subject, text, HTML, the unresolved variable list, `recipient_suppressed`, and attachments. It stores nothing. | backend/src/service/email.rs:346-357, 555-562 | documented |
| MAIL-14 | A manual send with any unresolved variable (MISSING or UNKNOWN) is refused with 422 "unresolved template variables: …", and nothing is queued. The reason: a person is present to fix it. | backend/src/service/email.rs:564-578; backend/src/api/email.rs:117 | documented |
| MAIL-15 | A manual send to a suppressed address is allowed. The preview returns `recipient_suppressed=true` so the UI can warn. Automatic mail to a suppressed address is never sent. | backend/migrations/0005_email.sql:121-122; backend/src/service/email.rs:354-355 | documented |
| MAIL-16 | `body_markdown=true` together with `template_key` is refused (422). The quotation letter refuses `body_markdown` when the body contains `{{`. The reason: values containing asterisks would format the letter by accident. | backend/src/service/email.rs:330-333, 492-496, 692-698 | documented |
| MAIL-17 | Sender identity: if the user's email domain is in `EMAIL_SENDER_DOMAINS`, the From is the user's own name and address, with no Reply-To. Otherwise the From is "<display name> – <from_name>" at `EMAIL_FROM_AUTOMATIC`, with Reply-To set to the user's own address. Replies never go to an unmonitored box. | backend/src/service/email.rs:195-211, 595-596, 1355-1374; docs/email-google-workspace.md:13-14 | documented |
| MAIL-18 | Regular attachments must be existing documents; otherwise 422 "attachments must be existing documents". When the email is about an order or a lead, every attachment must belong to that record; otherwise 422. A letter about nothing (standalone) may attach any company document. | backend/src/service/email.rs:505-534 | documented |
| MAIL-19 | An embedded image must be an existing document whose content type starts with `image/`, owned by the order or lead when there is one. Each `doc:ID` reference in the body must match a picked embed; otherwise 422 "doc:N is referenced but not embedded". A dangling `doc:N` with no embeds picked is also refused. | backend/src/service/email.rs:359-428; backend/src/domain/template.rs:228-287, 440-444 | documented |
| MAIL-20 | Embedded images are sent as inline parts under `cid:doc-ID` and are never turned into download links. In the text part, the reference is replaced by the filename. | backend/src/service/email.rs:340-343, 1147-1169; backend/src/domain/template.rs:228-236; backend/src/integrations/email.rs:423-435 | documented |
| MAIL-21 | A successful manual send returns 202 with the queued `EmailMessage` (status `queued`). Queued means "accepted for delivery", not "delivered". | backend/src/api/email.rs:115-131 | documented |
| MAIL-22 | Quotation (`POST /leads/{id}/quotation`, `SendEmail` required): the recipient is the lead's `contact_email`, falling back to the partner's email. With no address at all, the response is 422 "lead #N has no email address: add a contact email first". | backend/src/api/leads.rs:389-407; backend/src/service/email.rs:648-665 | documented |
| MAIL-23 | Default quotation letter: subject "Árajánlatunk: {{lead.title}}", hero band "Megjött az Autotherm árajánlatod!". The price line is included only if `lead.quoted_total` is known, and the validity line only if `lead.quote_valid_until` is known. A blank hero is refused with 422. Unresolved variables are refused. Attachments must be documents of this lead. | backend/src/service/email.rs:618-746 | documented |
| MAIL-24 | The quotation letter is sent as the staff member (same rule as MAIL-17), with `trigger='quotation'` and no idempotency key. | backend/src/service/email.rs:748-770; backend/src/api/leads.rs:389-391 | documented |
| MAIL-25 | Cancel (`POST /emails/{id}/cancel`): allowed for the author, or for anyone with `OperateSystem` (Admin). Anyone else gets 403. Only `queued`, `failed` and `needs_review` rows can be cancelled; `sending`, `sent` or `cancelled` gives 409 `not_cancellable`. Success returns 204. | backend/src/api/email.rs:133-157; backend/src/repo/emails.rs:274-290; backend/src/domain/role.rs:26 | documented |
| MAIL-26 | Retry (`POST /emails/{id}/retry`, `OperateSystem` only) accepts only `failed` or `needs_review` rows; anything else gives 409 `not_retryable`. It sets the row back to `queued` with `send_after=now()`, enqueues `send_email:{id}`, and returns 202. | backend/src/api/email.rs:159-177; backend/src/service/email.rs:1287-1310 | documented |

### Templates and placeholder rendering

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| MAIL-30 | Templates support only `{{path}}` substitution from a fixed whitelist: no expressions, loops or conditionals. The whitelist is listed in `VARIABLES` and served by `GET /email-templates/variables` with Hungarian descriptions. | backend/src/domain/template.rs:1-47; backend/src/api/email.rs:196-207 | documented |
| MAIL-31 | A whitelisted variable with no value, or a value that is empty after trimming, renders as the visible `{{MISSING:path}}`. A non-whitelisted variable renders as `{{UNKNOWN:path}}`. Both are listed in `unresolved`. | backend/src/domain/template.rs:5-6, 114-134, 351-372 | documented |
| MAIL-32 | Whitespace inside braces is ignored (`{{ order.number }}` works). Text in braces that is not a valid name (`a-z 0-9 _ .`, at most 64 characters), or an unterminated `{{`, stays literal and does not count as unresolved. | backend/src/domain/template.rs:58-90, 374-379 | documented |
| MAIL-33 | Substituted values are never rendered again: a value containing `{{order.number}}` appears literally. | backend/src/domain/template.rs:381-388 | documented |
| MAIL-34 | Subjects are single-line: every CR/LF from the template or from values becomes a space, which prevents header injection such as "Bcc:". | backend/src/domain/template.rs:136-142, 446-449; backend/src/service/email.rs:547, 995 | documented |
| MAIL-35 | For plain-text bodies, the HTML part is derived from the text. Everything, values included, is HTML-escaped (`& < > " '`). Blank lines separate paragraphs and single newlines become `<br>`. | backend/src/domain/template.rs:144-168, 197-212, 390-397 | documented |
| MAIL-36 | Markdown bodies render `{{variables}}` first and then Markdown. Staff-authored inline HTML passes through unescaped by design; the inbox must render stored HTML sandboxed. | backend/src/domain/template.rs:183-195, 409-415 | documented |
| MAIL-37 | Every letter uses the same branded, email-client-safe layout (tables and inline styles only, AUTOTHERM header, footer). The HTML ends with `</body></html>`, where footers, links and banners are spliced in. | backend/src/domain/template.rs:159-174, 289-328 | documented |
| MAIL-38 | Creating a template (`POST /email-templates`) or editing one (`PATCH /email-templates/{id}`) requires `ManageConfiguration` (Admin). The key must match `^[a-z][a-z0-9_]*$` and be unique. Name, subject and body are required. Any unknown variable is refused with 422 "unknown template variables: …". Every create or edit is written to the audit log. | backend/src/api/email.rs:209-321; backend/migrations/0005_email.sql:5 | documented |
| MAIL-39 | A template's `key` and `is_automatic` never change after creation. Code refers to templates by key, and whether a template is sent without a human is a property of the code path. | backend/src/repo/templates.rs:72-73 | documented |
| MAIL-40 | Seeded templates: `blocker_nudge_first`, `blocker_nudge_escalated`, `lead_acknowledgement` (manual), `order_stage_changed`, `stalled_order_alert` (0005); `order_ready_for_pickup` (0016); `invoice_issued`, `invoice_stornoed`, `invoice_annulled`, `proforma_created` (0022). The seeds are idempotent (`ON CONFLICT (key) DO NOTHING`), and the wording can be edited afterwards. | backend/migrations/0005_email.sql:16-68; backend/migrations/0016_pickup_template.sql:5-14; backend/migrations/0022_invoice_email_templates.sql:1-3, 7-67 | documented |
| MAIL-41 | The `proforma_created` letter must say that a díjbekérő is a payment request, not a tax document: it does not allow VAT deduction and is not reported to NAV. | backend/migrations/0022_invoice_email_templates.sql:4-6, 63 | documented |
| MAIL-42 | Values: dates render as `YYYY.MM.DD.` and money as a `Money` display with its currency (default HUF). `order.vehicle` is make, model and plate joined by spaces. `blocker.days_overdue` is set only when the blocker is overdue by more than 0 days. `order.stage` is the Hungarian stage label. `contact.name` comes from the order's contact, else the lead's contact name. `partner.name` comes from about → order → lead. | backend/src/service/email.rs:175-182, 213-317 | documented |
| MAIL-43 | Invoice and proforma variables are not derivable from an order (an order can have several invoices). The caller supplies them as `extra_values`, which override the derived values. | backend/src/domain/template.rs:25-27; backend/src/service/email.rs:955-960, 993-994 | documented |

### Automatic mail: triggers, safety rails, idempotency

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| MAIL-50 | Automatic mail is rendered and queued inside the caller's transaction. Unresolved variables stay visible in the stored row, and the send step fails the row rather than mailing a broken message. | backend/src/service/email.rs:978-980 | documented |
| MAIL-51 | At delivery, automatic mail passes these rails in order: (1) kill switch off → `cancelled` "automatic email is switched off"; (2) recipient suppressed → `cancelled`; (3) MISSING or UNKNOWN marker in the subject or text → `failed` "template has unresolved variables"; (4) outside the send window → stays `queued` with `send_after` = next opening; (5) at least `max_auto_emails_per_recipient_day` automatic mails sent or sending to this address in the last 24h → `cancelled` "recipient rate limit reached". Manual mail skips all of these. | backend/src/domain/email.rs:72-115, 211-264; backend/src/service/email.rs:1080-1120; backend/src/repo/emails.rs:300-309 | documented |
| MAIL-52 | Send window: the defaults are 08:00-17:00, weekdays only, in the business time zone (Europe/Budapest), and it follows DST. Mail at night defers to the next opening; mail on Saturday, Sunday or Friday evening defers to Monday 08:00. The end time is exclusive (17:00 counts as closed). | backend/migrations/0001_foundation.sql:69-71; backend/src/domain/email.rs:33-70, 172-209 | documented |
| MAIL-53 | The automatic kill switch (`automatic_email_enabled`) is off by default and must be turned on deliberately. Changing it logs a warning and writes the full before/after settings to the audit log. | backend/migrations/0001_foundation.sql:66-67; backend/src/api/configuration.rs:610-625; docs/email-google-workspace.md:112-113 | documented |
| MAIL-54 | Each automatic send is queued exactly once per business event, because `email_messages.idempotency_key` is UNIQUE and a duplicate insert returns None with no job: nudge `nudge:{blocker}:{n}`, stage `stage:{stage_row_id}`, pickup `pickup:{stage_row_id}`, stalled `stalled:{order}:{entered_at}:{recipient}:{YYYY}-W{ww}`, invoice `{trigger}:{invoice_id}`, proforma `proforma:{proforma_id}`. Manual, quotation and newsletter mail have no key. | backend/migrations/0005_email.sql:86-87; backend/src/repo/emails.rs:87-95; backend/src/domain/blocker.rs:53-57; backend/src/service/automation.rs:85, 128-136, 216, 255; backend/src/service/invoicing.rs:1146-1147, 1189 | documented |
| MAIL-55 | Blocker nudges are queued only when the kill switch is on and the blocker is unresolved, has nudging enabled and has a due date, and today is after the due date. A repeat nudge also needs at least `nudge_interval_days` (default 3) since the last one. The escalated template is used once `nudge_count >= nudge_escalate_after` (default 2). A blocker with no recipient address is skipped with a warning. | backend/src/service/automation.rs:22-116; backend/src/domain/blocker.rs:14-51; backend/migrations/0001_foundation.sql:72-73 | documented |
| MAIL-56 | The nudge count is re-checked under a row lock. If another worker nudged in the meantime, nothing is queued. A queued nudge also records `record_nudge` plus an audit entry `blocker_nudge` in the same transaction. | backend/src/service/automation.rs:60-105 | documented |
| MAIL-57 | Stalled-order alerts go to each address in `stalled_alert_recipients`, at most once per order, per stage visit, per recipient, per ISO week. Nothing is sent when the kill switch is off or the list is empty. The job is scheduled daily after 07:00 local time. | backend/src/service/automation.rs:118-167; backend/src/jobs/mod.rs:214-222 | documented |
| MAIL-58 | Customer stage mail is queued only when `stage_change_notifications` and the kill switch are both on, and the order has a customer address (contact email, else partner email). It is sent only for forward moves: rework and cancellation are not announced. Entering `completed` sends `order_ready_for_pickup` instead of the generic mail: one letter, not two. | backend/src/service/stages.rs:157-163; backend/src/service/automation.rs:169-262; backend/migrations/0016_pickup_template.sql:1-4 | documented |
| MAIL-59 | Invoice letters: `invoice_issued` for an invoice, `invoice_stornoed` for a storno, `invoice_annulled` when the status is Annulled. The recipient is the customer address from MAIL-58; with no address, the letter is skipped with a warning. The invoice PDF is attached, except on annulment, which carries no document. `proforma_created` attaches the proforma PDF. | backend/src/service/invoicing.rs:1085-1200 | documented |
| MAIL-60 | Automatic mail is sent from `EMAIL_FROM_NAME <EMAIL_FROM_AUTOMATIC>` with Reply-To `EMAIL_REPLY_TO_DEFAULT`. It carries `Auto-Submitted: auto-generated` and `X-Auto-Response-Suppress: All`, so vacation responders don't answer; manual mail does not carry them. | backend/src/service/email.rs:998, 1021; backend/src/integrations/email.rs:41-42, 59-89, 344-346, 406-414; docs/email-google-workspace.md:10-12 | documented |
| MAIL-61 | `settings.max_auto_emails_per_recipient_day` defaults to 3 and must be ≥ 0. A value of 0 blocks all automatic mail. | backend/migrations/0001_foundation.sql:68; backend/src/api/configuration.rs:566-571 | implied |

### Delivery, attachments, failure handling

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| MAIL-70 | Delivery locks the row. Only a `queued` row is sent. A row found in `sending` (a previous attempt died mid-flight) becomes `needs_review` with "a previous send attempt was interrupted…" and is never resent automatically. Any other status is a no-op. | backend/src/service/email.rs:1048-1075; backend/src/domain/email.rs:19-21 | documented |
| MAIL-71 | A row whose `send_after` is in the future is not sent; the job is deferred to `send_after`. | backend/src/service/email.rs:1076-1078 | documented |
| MAIL-72 | If any attached document was deleted before sending, the row becomes `failed` "an attached document was deleted before sending" and nothing is sent. | backend/src/service/email.rs:1122-1141 | documented |
| MAIL-73 | If the regular attachments total at most 10 MiB, they are attached. Above that, they become presigned download links valid for 7 days, listed under "Letölthető fájlok (7 napig érvényes):" in both parts. The stored bodies are updated so the log shows what was actually sent. After sending, each attachment ref records its mode: `attached`, `link` or `embedded`. | backend/src/service/email.rs:44-45, 1142-1224, 1162-1168; backend/src/repo/emails.rs:222-238 | documented |
| MAIL-74 | Attachments are resolved before the row is committed as `sending`, so a storage failure retries cleanly. The row is committed as `sending` (attempts+1, `sending_started_at`) before SMTP is contacted. | backend/src/service/email.rs:1122, 1226-1227; backend/src/repo/emails.rs:195-203 | documented |
| MAIL-75 | Each send gets a Message-ID `<email-{id}.{random hex}@{EMAIL_MESSAGE_ID_DOMAIN}>`, stored as `provider_id` on success together with `status='sent'`, `sent_at`, and the error cleared. | backend/src/service/email.rs:1229-1234, 1251-1256; backend/src/repo/emails.rs:205-220 | documented |
| MAIL-76 | SMTP outcomes: a permanent error (5xx, bad address, unbuildable message) → `failed` with no retry. A transient error (4xx, connection refused, DNS) → requeued with backoff, until 5 send attempts, then `failed` "gave up after N attempts: …". A timeout mid-conversation (outcome unknown) → `needs_review` with no automatic retry, to avoid duplicates. | backend/src/integrations/email.rs:45-57, 229-240; backend/src/service/email.rs:46, 1257-1284 | documented |
| MAIL-77 | Failed rows keep the server's answer in `error`, plus a plain-language hint for common Google relay errors: 5.7.0 relay denied, 5.7.1, 535/5.7.8 auth, 421/4.7.0, certificate, and unreachable (including localized Windows os-error codes). | backend/src/integrations/email.rs:268-322, 455-493; docs/email-google-workspace.md:116-127 | documented |
| MAIL-78 | Dry-run mode builds the message (so invalid mail still fails permanently) and logs it without sending. The row still ends up `sent`. | backend/src/integrations/email.rs:196-210, 495-498; backend/src/service/email.rs:1252-1254 | documented |

### Transport settings and redirect (0007)

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| MAIL-80 | Transport columns in `settings` that are NULL mean "inherit from the environment". When `email_mode` is set, the database owns the whole transport, including `redirect_to`, and the environment is ignored. `mode: null` (or blank) in `PUT /settings` resets everything to the environment. | backend/migrations/0007_settings_email.sql:1-5; backend/src/api/configuration.rs:406-481; backend/src/service/email.rs:120-147 | documented |
| MAIL-81 | The SMTP password is stored in the clear in the database but is never returned by the API; only `has_password` is exposed. In `PUT /settings`, an absent password keeps the stored one, null clears it, and a value replaces it. | backend/migrations/0007_settings_email.sql:6-8; backend/src/api/configuration.rs:409-410 | documented |
| MAIL-82 | `PUT /settings` requires `ManageSettings` (Admin). The send window start must be before its end. Limits must be positive. Stalled-alert recipients must be valid addresses. SMTP mode needs a host; the port must be 1-65535; security must be one of none/starttls/tls; the HELO name must contain no whitespace; `redirect_to` must be a valid address. | backend/src/api/configuration.rs:483-579; backend/migrations/0007_settings_email.sql:10-13 | documented |
| MAIL-83 | Outside production, SMTP mode must target a local sink (localhost, 127.0.0.1, ::1, mailpit) or set a redirect. Test mail must never reach real customers. This is enforced both at startup (env) and on `PUT /settings`. | backend/src/config.rs:188-210, 410-416; backend/src/api/configuration.rs:526-528; docs/email-google-workspace.md:106-110 | documented |
| MAIL-84 | With a redirect active, every message goes only to the redirect address. The subject is prefixed `[TESZT – eredeti címzett: <to>, cc: …]`, and a banner is added to the text and HTML. The original CC recipients are dropped. | backend/src/integrations/email.rs:242-266, 437-453; docs/email-google-workspace.md:106-110 | documented |
| MAIL-85 | Broken stored transport rows fall back to the environment with an error log and never stop sending: an unreadable table, an SMTP row with a blank host, or an unknown security value. The worker rebuilds the transport every cycle, so changes apply without a restart. Defaults: port 587, STARTTLS; HELO falls back to `EMAIL_MESSAGE_ID_DOMAIN`. | backend/src/service/email.rs:57-147, 1401-1439; backend/src/jobs/mod.rs:51-68 | documented |
| MAIL-86 | `POST /admin/email/test` (`OperateSystem`) sends one test mail through the saved transport, or through an unsaved candidate that is validated but never persisted. It bypasses the queue and always answers 200 with `ok`/`detail`. | backend/src/api/admin.rs:231-330 | documented |
| MAIL-87 | TLS always verifies the certificate against the configured host name, even when `force_ipv4` connects to a resolved IP. | backend/src/integrations/email.rs:110-116 | documented |

### Suppression list

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| MAIL-90 | Suppressed addresses are stored lowercase (enforced by a CHECK). The lookup is case-insensitive. | backend/migrations/0005_email.sql:123-128; backend/src/repo/emails.rs:311-318 | documented |
| MAIL-91 | Adding a suppression (`POST /email-suppressions`, `SendEmail`) validates and normalizes the address. Re-adding updates the reason and returns 204. Listing requires `SendEmail`. | backend/src/api/email.rs:323-357; backend/src/repo/emails.rs:337-353 | documented |
| MAIL-92 | Removing a suppression (`DELETE /email-suppressions/{email}`) requires `ManageConfiguration` (Admin), because it re-enables automatic mail to someone who asked not to receive it. An unknown address gives 404. The removal is logged as a warning. | backend/src/api/email.rs:359-379 | documented |
| MAIL-93 | Suppression is checked at delivery time, not at queue time. An address suppressed after queueing but before sending is still honoured for automatic mail (→ cancelled). | backend/src/service/email.rs:1088-1090; backend/migrations/0005_email.sql:121-122 | documented |

### Newsletter

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| NEWS-01 | A newsletter blast is one `email_messages` row with `trigger='newsletter'`. The To is the company's own automatic mailbox, and all recipients are in `bcc`, so no recipient sees the list. | backend/migrations/0024_newsletter.sql:8-11, 25; backend/src/service/email.rs:786-791, 906-931 | documented |
| NEWS-02 | Recipients are active subscribers (`unsubscribed_at IS NULL`), lowercased and deduplicated, minus every address on the global suppression list. If none remain, the response is 422 "nobody to send to…". | backend/src/repo/newsletter.rs:236-245; backend/src/service/email.rs:834-846 | documented |
| NEWS-03 | Newsletter subject and body are literal. Any `{{variable}}` (known or unknown) is refused with 422 "a newsletter has no recipient to resolve … against". Blank subject or body is refused. | backend/src/service/email.rs:786-791, 814-832 | documented |
| NEWS-04 | Every blast carries an unsubscribe footer with the link `{PUBLIC_BASE_URL}/hu/newsletter/unsubscribe`, in both the text part ("Leiratkozás: …") and the HTML part. | backend/src/service/email.rs:848-869 | documented |
| NEWS-05 | Sending a blast requires `SendEmail`. It returns 202 `{email_id, recipients}`, so the office knows how many addresses were included. The blast may attach or embed any company document (no ownership check). | backend/src/api/newsletter.rs:86-107; backend/src/service/email.rs:800-806, 871-900 | documented |
| NEWS-06 | A blast is sent as manual mail (`sent_by` = the user, `is_automatic=false`) from `EMAIL_FROM_NAME <EMAIL_FROM_AUTOMATIC>` with Reply-To `EMAIL_REPLY_TO_DEFAULT`. The automatic rails in MAIL-51 therefore do not apply to it. | backend/src/service/email.rs:902-925; backend/src/repo/emails.rs:102 | implied |
| NEWS-07 | Subscriptions: the email is UNIQUE and non-empty, `source` is `office` (hand-added) or `website` (public signup), and each subscription has a random 48-hex-character `unsubscribe_token`. Matching is case-insensitive. | backend/migrations/0024_newsletter.sql:1-7, 15-24; backend/src/repo/newsletter.rs:1-7 | documented |
| NEWS-08 | Subscribing is idempotent. Subscribing again updates the name and clears `unsubscribed_at`: coming back counts as saying yes again. | backend/src/api/newsletter.rs:109-110; backend/src/repo/newsletter.rs:247-272 | documented |
| NEWS-09 | Unsubscribing sets `unsubscribed_at` and does not delete the row, so a later import cannot resubscribe someone who opted out. The office list shows unsubscribed rows too. | backend/migrations/0024_newsletter.sql:4-6; backend/src/api/newsletter.rs:32-33 | documented |
| NEWS-10 | Public signup (`POST /newsletter/subscribe`) requires the header `X-Newsletter-Key` to equal `NEWSLETTER_API_KEY`. When the key is unset, signup is off and returns 403. An invalid address gives 422. Success returns 201. | backend/src/api/newsletter.rs:3-6, 109-129, 178-191 | documented |
| NEWS-11 | Public unsubscribe (`GET /newsletter/unsubscribe?token=` or `?email=`) needs no login. It always answers 200 `{unsubscribed: bool}`: true only if an active subscription changed. A second click or an unknown address returns false and never an error, so guessing addresses reveals nothing. A token takes precedence over an email. | backend/src/api/newsletter.rs:131-176 | documented |
| NEWS-12 | Hand-adding (`POST /newsletter/subscriptions`) and deleting (`DELETE /newsletter/subscriptions/{id}`) require `SendEmail`. An unknown id gives 404. | backend/src/api/newsletter.rs:52-84 | documented |

### Background job queue

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| JOB-01 | Jobs are claimed with `FOR UPDATE SKIP LOCKED`, in batches of 5, ordered by `run_at, id`. The worker polls every 5 s when idle. Claiming increments `attempts`. | backend/src/jobs/mod.rs:1-5, 35-39, 69-87; backend/src/repo/jobs.rs:308-331 | documented |
| JOB-02 | A job locked longer than the 15-minute lease is treated as abandoned and can be claimed again. The per-job timeout (600 s) must stay below the lease. | backend/src/repo/jobs.rs:251-253; backend/src/jobs/mod.rs:37-38 | documented |
| JOB-03 | By default a job may run 5 times (`max_attempts=5`). A failure with attempts < max is rescheduled with backoff 30s·2^(attempts-1) (30 s, 1 m, 2 m, 4 m…), capped at 6 h. At the maximum, the job is dead-lettered (`failed_at` set). | backend/migrations/0001_foundation.sql:92, 96; backend/src/jobs/mod.rs:100-113; backend/src/repo/jobs.rs:343-363, 427-444 | documented |
| JOB-04 | Deferring a job (the send window, or `send_after` in the future) does not count as a failed attempt: attempts are decremented back. | backend/src/repo/jobs.rs:365-375; backend/src/jobs/mod.rs:102 | documented |
| JOB-05 | At most one unfinished job exists per `dedupe_key`, enforced by a partial unique index. A second enqueue with the same key is silently skipped. Send jobs use `send_email:{email_id}`, so one email is never queued twice at once. | backend/migrations/0001_foundation.sql:88, 101-102; backend/src/repo/jobs.rs:273-295 | documented |
| JOB-06 | Periodic jobs are scheduled each minute with period keys, so each exists exactly once per period even across several instances: nudges hourly (`nudge_blockers:YYYY-MM-DDTHH`), stalled alerts daily from 07:00, FX rates daily from 12:30. | backend/src/jobs/mod.rs:3-5, 197-242; backend/src/repo/jobs.rs:297-306 | documented |
| JOB-07 | Admin (`OperateSystem`) can list failed or pending jobs, retry a dead-lettered job (fresh attempt budget; only dead jobs, else 409 `not_failed`), and run `nudge_blockers` or `stalled_orders` immediately. | backend/src/api/admin.rs:44-72, 125-167; backend/src/repo/jobs.rs:415-425 | documented |
| JOB-08 | Claimed jobs are finished even if shutdown arrives, rather than abandoned until the lease expires. If the outcome cannot be recorded, the lease expires and the job runs again. | backend/src/jobs/mod.rs:83-87, 115-117 | documented |
| JOB-09 | A misconfigured email transport skips that worker cycle with an error log. It never stops the worker. | backend/src/jobs/mod.rs:51-68 | documented |

### Web client (compose, inbox, detail, newsletter, unsubscribe)

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| MAIL-100 | Compose, quotation and newsletter controls are shown only to Admin and Office (`canSendEmail` = `canEdit`), matching the backend `SendEmail` capability. The newsletter list component renders nothing for other roles. | frontend/src/lib/auth/context.tsx:74-76, 102-105; frontend/src/components/email/NewsletterList.tsx:55 | implied |
| MAIL-101 | The compose preview re-renders on the server after a debounce whenever to, cc, template, subject, hero, body, markdown, attachments or embeds change. A preview failure does not retry. The preview is shown in a sandboxed iframe (`sandbox=""`), so staff HTML cannot run scripts. | frontend/src/components/email/ComposeForm.tsx:184-210, 466-470; backend/src/domain/template.rs:185-186 | documented |
| MAIL-102 | When the preview reports `recipient_suppressed` or unresolved variables, the form shows a note ("Címzett tiltva van", "Feloldatlan változók: …") before sending. Send stays enabled, but the backend refuses unresolved variables (MAIL-14). | frontend/src/components/email/ComposeForm.tsx:458-464; frontend/src/messages/hu.json:454-455 | implied |
| MAIL-103 | Picking a template fills subject and body, turns Markdown off and clears any theme. Picking a theme clears the template and turns Markdown on. This keeps the client from sending the template + Markdown combination the server refuses. | frontend/src/components/email/ComposeForm.tsx:153-174; backend/src/service/email.rs:492-496 | documented |
| MAIL-104 | Send is disabled while a send is pending, when subject or body is blank, and when there is no recipient: an empty To for direct mail, or zero active subscribers for a newsletter. A failed send shows the server's error message inline, and the form keeps its content. | frontend/src/components/email/ComposeForm.tsx:216-243, 364-366, 446 | documented |
| MAIL-105 | The attachment picker offers only documents of the order or lead the letter is about. For a newsletter or standalone letter it offers the recent-documents library. This mirrors the backend ownership rule. Picking an embed inserts `![filename](doc:ID)` into the body. | frontend/src/components/email/ComposeForm.tsx:93-137; backend/src/service/email.rs:505-541 | documented |
| MAIL-106 | Inbox: statuses show as Várakozik/Küldés folyamatban/Küldve/Sikertelen/Visszavonva/Átnézésre vár. The inbox filters by status and free text, and CSV export covers the first 200 rows of the current status filter. | frontend/src/app/[locale]/emails/page.tsx:31-52, 154-170; frontend/src/messages/hu.json:446-451 | documented |
| MAIL-107 | Email detail shows the stored `error`. For `needs_review`, it warns that the delivery outcome is unknown and must be checked with the provider before any retry. The HTML body renders in a sandboxed iframe. Cancel and Retry each ask for confirmation ("A várakozó levél nem kerül kiküldésre" / "Újraküldi a levelet?"). | frontend/src/app/[locale]/emails/[id]/page.tsx:180-197, 274, 284-290; frontend/src/messages/hu.json:457, 464-465 | documented |
| MAIL-108 | Web shows Retry only to Admins on `failed`/`needs_review` rows, and Cancel only on `queued` rows for the author or an Admin. The backend remains the enforcer. | frontend/src/app/[locale]/emails/[id]/page.tsx:115-122 | documented |
| MAIL-109 | The quotation dialog pre-fills the hero "Megjött az Autotherm árajánlatod!". Empty subject, hero or body means the backend defaults are used. On success it shows the toast `quotationQueued`. | frontend/src/components/email/QuotationDialog.tsx:3-4, 37, 49-55, 66-79 | documented |
| MAIL-110 | The public unsubscribe page needs no login and no app shell. A `?token=` link unsubscribes once, as soon as the page opens. Without a token, the reader types an address. `unsubscribed=true` shows "Leiratkoztattuk…"; `false` shows "Ez a cím már le van iratkozva — nincs teendő." | frontend/src/app/[locale]/newsletter/unsubscribe/page.tsx:1-80; frontend/src/messages/hu.json:509-511 | documented |
| MAIL-111 | The newsletter list shows every subscription with a subscribed or unsubscribed badge and "active/total" counts. Removing a subscription asks for confirmation first. | frontend/src/components/email/NewsletterList.tsx:46-63, 110-138 | documented |
| NEWS-13 | Newsletter compose shows "{count} feliratkozó kapja meg BCC-ben." The client counts all subscriptions without `unsubscribed_at` (suppressed addresses included). The server's 202 response gives the actual number of recipients. | frontend/src/components/email/ComposeForm.tsx:143-151, 267-270; frontend/src/messages/hu.json:475; backend/src/api/newsletter.rs:86-92 | implied |

### Android client

| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| MAIL-120 | Android compose is deliberately minimal: to, subject and body only. There are no templates, heroes or attachments, which stay on the desktop. Its gaps are covered by finding C7. | android/app/src/main/java/hu/autotherm/autocrm/ui/emails/EmailComposeScreen.kt:37-41 | documented |
| MAIL-121 | The compose entry points (the order's "Levél" action and the email list's FAB) are shown only when `canEdit` (admin/office), matching `SendEmail`. | android/.../ui/orders/OrderDetailScreen.kt:351-353; android/.../ui/emails/EmailScreens.kt:135-136; android/.../data/auth/SessionStore.kt:54 | implied |
| MAIL-122 | Prefill is a courtesy and a failure must not block composing. For an order, To = the partner's email and Subject = "#number · title". For a lead, To = the lead's contact email and Subject = the lead title. | android/.../ui/emails/EmailComposeScreen.kt:55-80 | documented |
| MAIL-123 | Send is disabled while busy or when To is blank. Blank subject or body is sent as null, so a missing subject or body comes back as the server's 422 and appears in red under the form. The form keeps its content. | android/.../ui/emails/EmailComposeScreen.kt:87-107, 153-161 | documented |
| MAIL-124 | The Android list and detail show a status badge (Várakozik/Küldés/Elküldve/Sikertelen/Visszavonva/Ellenőrzendő; `failed` and `needs_review` in the signal colour) and the stored error. Detail shows the plain-text body, which is what was actually sent, rather than rendered HTML. | android/.../ui/emails/EmailScreens.kt:95-110, 164-180, 266-296 | documented |

<a id="reports"></a>

## Reports, dashboard, search & time

**Scope.** This journey covers:
- **Server reports:** `GET /api/reports/{volume,stage-durations,throughput,workload,stalled,blocker-load,fx-rates}`, in `backend/src/api/reports.rs`, `backend/src/repo/reports.rs` and the views in `backend/migrations/0006_reporting.sql`.
- **Web reports page:** `frontend/src/app/[locale]/reports/page.tsx` → `components/reports/WorkloadCharts.tsx` and `WorkloadChartsView.tsx`.
- **Android reports screen:** `android/app/src/main/java/hu/autotherm/autocrm/ui/reports/ReportsScreen.kt`.
- **Web dashboard:** `frontend/src/components/dashboard/DashboardView.tsx`, built on the client from six queries.
- **Global search:** `GET /api/search` in `backend/src/api/search.rs` and `backend/src/repo/search.rs`; on the web, `components/search/GlobalSearch.tsx` and `CommandPalette.tsx`.
- **List pagination:** `backend/src/api/mod.rs`, `backend/src/repo/mod.rs`, `frontend/src/components/ui/Pagination.tsx`.
- **"Today" and "overdue":** `backend/src/service/mod.rs`, `repo/blockers.rs`, `domain/blocker.rs`; web `components/forms/DateQuickPicks.tsx`; Android `util/Format.kt`.
- **Settings, admin and audit:** `api/configuration.rs`, `repo/config.rs`, `api/auth.rs` (preferences), `0001_foundation.sql`, `0008_user_settings.sql`, `api/admin.rs`, `repo/audit.rs`.
- **MiniCRM migration:** `backend/src/migration/*`, `api/raw_import.rs`, `docs/migration/README.md`.

Related findings already filed and referenced by ID only: A5, C6, C8, C9, C10, C11, F3.

### Report periods and common rules
| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| REP-01 | Every report needs a signed-in user. Any role (admin, office, designer, viewer) may read reports. A request with no session gets 401 `unauthenticated`. | backend/src/api/reports.rs:95; docs/API.md:28 | documented |
| REP-02 | If `to` is left out, it defaults to today in Budapest (the business time zone), not the UTC date. | backend/src/api/reports.rs:39,50; backend/src/service/mod.rs:15-17 | documented |
| REP-03 | If `from` is left out on volume, stage-durations, throughput, blocker-load or fx-rates, it defaults to `to − 365 days`, i.e. "the last 12 months up to today, inclusive". | backend/src/api/reports.rs:36,48,51; docs/API.md:166 | documented |
| REP-04 | If `from > to`, the request fails with 400 `validation` and the message "from must not be after to". `from == to` is a valid one-day period. | backend/src/api/reports.rs:52-54,313-315 | documented |
| REP-05 | Both the `from` and `to` days are included. Reports that filter on timestamps use the half-open range [Budapest midnight of `from`, Budapest midnight of `to+1`), converted to UTC. | backend/src/api/reports.rs:48,58-67 | documented |
| REP-06 | On a DST switch day, the bound is still that Budapest day's midnight. A 23-hour or 25-hour day counts as exactly one day, with no gap or overlap between back-to-back periods. | backend/src/api/reports.rs:58-66 | implied |
| REP-07 | Every report response repeats the period it actually used after defaults (`period.from`/`period.to`, or `from`/`to` for workload). | backend/src/api/reports.rs:42-46,129-131,292-297 | implied |
| REP-08 | Report durations are fractional days (seconds ÷ 86400), not counts of calendar days. | backend/src/repo/reports.rs:1-2,132-134 | documented |

### Volume (`/reports/volume`)
| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| REP-10 | An order is in the period if its `valuation_date` (a date, not `created_at`) falls between `from` and `to`, both included. | backend/src/repo/reports.rs:39; docs/DECISIONS.md:25-28 | documented |
| REP-11 | The valuation date defaults to the day the order was created and can be edited. It is never "today at report time". | docs/DECISIONS.md:25-28 | documented |
| REP-12 | `group` accepts only `month` (the default), `partner` or `project_type`. Any other value gets 400 `validation` "unknown group '<x>'". | backend/src/api/reports.rs:74,99-108; docs/API.md:170 | documented |
| REP-13 | By default, orders whose current stage is an exit stage (cancelled) are left out; `include_cancelled=true` brings them in. Completed orders (terminal but not exit) are always counted. | backend/src/repo/reports.rs:39; backend/src/api/reports.rs:76-77 | documented |
| REP-14 | Each row gives: number of orders, `huf_minor` (sum of HUF orders), `eur_minor` (sum of EUR orders), `normalized_huf_minor` (everything converted to HUF), and `missing_fx` (orders that could not be converted to HUF). All money values are integer minor units. | backend/src/repo/reports.rs:9-19; docs/API.md:3-4,170 | documented |
| REP-15 | An order's total is the sum over its line items of `round(quantity × unit_price)`, rounded half away from zero. An order with no line items totals 0 but still counts as one order. | backend/migrations/0006_reporting.sql:54-58; docs/DECISIONS.md:36-38 | documented |
| REP-16 | HUF orders keep their own amount. Other currencies convert at the latest MNB rate (currency → HUF) dated on the valuation date or up to 10 days before it: `round(total × rate)`. | 0006_reporting.sql:38-41,49-52,59-69; docs/DECISIONS.md:106-108 | documented |
| REP-17 | If no rate falls in that 10-day window, the order's HUF value is null. It is left out of `normalized_huf_minor` and counted in `missing_fx`. Today's rate is never used as a stand-in. | 0006_reporting.sql:39-41; docs/DECISIONS.md:106-108; docs/history/VIABILITY.md:47 | documented |
| REP-18 | With `group=month`, rows are keyed and labelled by the valuation month (`YYYY-MM`) and sorted by date. Months with no orders have no row. | backend/src/repo/reports.rs:29,40 | implied |
| REP-19 | With `group=partner`, there is one row per partner (key = partner id, label = partner name), sorted by HUF value (highest first), then by name. | backend/src/repo/reports.rs:57,69 | implied |
| REP-20 | With `group=project_type`, there is one row per project type, plus a row keyed `unassigned` and labelled "(nincs megadva)" for orders with no type. Rows are sorted by HUF value (highest first), then by label. | backend/src/repo/reports.rs:86,98 | implied |
| REP-21 | `totals` is an extra row (key `total`, label "Összesen"). Each of its numbers is the sum of that column over `rows`, so `totals.orders` equals the number of orders in the period whatever the grouping. | backend/src/api/reports.rs:110-128 | documented |

### Stage durations (`/reports/stage-durations`)
| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| REP-25 | A stage visit counts if the order entered the stage inside the Budapest-day bounds of the period. | backend/src/repo/reports.rs:138; backend/src/api/reports.rs:162 | documented |
| REP-26 | Terminal stages (completed, cancelled) are not reported. | backend/src/repo/reports.rs:138 | implied |
| REP-27 | `visits` counts all visits that started in the period. `currently_in_stage` counts the ones still open. | backend/src/repo/reports.rs:112-114,130-131; 0006_reporting.sql:27-36 | documented |
| REP-28 | Average, median and p90 are computed over finished visits only. A stage whose visits are all still open returns null for these, not 0. | backend/src/repo/reports.rs:115; docs/DECISIONS.md:104-105; docs/API.md:171 | documented |
| REP-29 | A visit ends when the same order enters its next stage (ordered by `entered_at`, then `id`). Coming back to a stage starts a new visit. | 0006_reporting.sql:27-36 | documented |
| REP-30 | `project_type_id` limits the report to that project type; leaving it out covers all orders. Rows are sorted by stage position. | backend/src/repo/reports.rs:139,141 | documented |

### Throughput (`/reports/throughput`)
| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| REP-33 | An order counts as completed if its current stage is terminal but not an exit stage, and it entered that stage within the period. Cancelled orders never count. | backend/src/repo/reports.rs:167-173 | documented |
| REP-34 | Completions are grouped by the Budapest calendar month they happened in (`YYYY-MM`), not the UTC month. | backend/src/repo/reports.rs:175; backend/src/api/reports.rs:185 | documented |
| REP-35 | Lead time is the time from the order's first stage entry to completion, in fractional days. The report gives the median and average per month. | backend/src/repo/reports.rs:154,169,177-178; docs/API.md:172 | documented |

### Workload (`/reports/workload`) on the server, web and Android
| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| REP-40 | By default `to` is today in Budapest and `from` is `to − 6`, i.e. 7 days including both ends. | backend/src/api/reports.rs:275-278,311-312 | documented |
| REP-41 | If `to − from` is more than 62 days, the request gets 400 "workload range caps at 62 days". A difference of exactly 62 (63 days) is allowed. | backend/src/api/reports.rs:300,316-318 | documented |
| REP-42 | The response has exactly one entry per day from `from` to `to` inclusive, oldest first, including days where every count is 0. | backend/src/api/reports.rs:321-340 | documented |
| REP-43 | `placed` for day D is the number of orders created on Budapest day D. | backend/src/api/reports.rs:284,324; backend/src/repo/reports.rs:263-265,280 | documented |
| REP-44 | `completed` for day D is the number of orders that reached a terminal stage on Budapest day D. | backend/src/api/reports.rs:286,325-328 | documented |
| REP-45 | `in_workshop` for day D counts orders placed on or before D and not completed before D. An order completed on D still counts on D. An order that is never completed counts on every day from the day it was placed. | backend/src/api/reports.rs:288,329-332 | documented |
| REP-46 | The web range picker offers: week (today−6 to today), month (today−29 to today) and month-to-date (the 1st of the current Budapest month to today). Both ends are Budapest dates. Week is the default. | frontend/src/components/reports/WorkloadCharts.tsx:3-5,46-54,58 | documented |
| REP-47 | The web totals are: total placed (sum of `placed`), total completed (sum of `completed`), and average in workshop (mean of `in_workshop` over the returned days, one decimal place, 0 when there are no days). | WorkloadCharts.tsx:77-81,111 | documented |
| REP-48 | The web shows a bar chart of placed and completed per day and an area chart of in-workshop per day. X-axis ticks read `MM.DD.` and the y-axis shows whole numbers only. The totals appear before the chart library has loaded. | WorkloadChartsView.tsx:3-11,28-31,42-58,67-85 | documented |
| REP-49 | The Android screen ("Jelentések", subtitle "Utolsó 30 nap") uses to = today and from = today−29, and shows the same three totals ("Beérkezett", "Elkészült", "Átlag bent", one decimal). | ReportsScreen.kt:44-47,71-72,104-105,119-151 | documented |
| REP-50 | Android's "today" for that window should be the Budapest date, as on the server and the web (the expected rule for A5). | backend/src/service/mod.rs:15; WorkloadCharts.tsx:46-48; Format.kt:11 | implied |

### Stalled orders (`/reports/stalled`)
| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| REP-55 | An order is stalled when its current stage is not terminal, that stage has a `stall_after_days` value, and the order entered the stage more than `stall_after_days` × 24 hours ago. A stage without `stall_after_days` never stalls. | backend/src/repo/reports.rs:215-216; docs/API.md:173 | documented |
| REP-56 | Each row gives the order number, title, partner, stage label, `entered_at`, whole days in the stage (rounded down), the stall threshold, and the number of open blockers. The oldest stage entry comes first. | backend/src/repo/reports.rs:190-201,206-217 | documented |
| REP-57 | The stalled report takes no period and always shows the state right now. | backend/src/api/reports.rs:189-198 | documented |
| REP-58 | Android sorts stalled rows by days in stage (longest first), shows "N napja" and a "N akadály" badge when there are open blockers, shows "Nincs beragadt munka." when the list is empty, and opens the order on tap. | ReportsScreen.kt:78,154-175 | documented |
| REP-59 | Stalled-order alert emails go out only when automatic email is on and at least one recipient is configured. Each order gets at most one alert per stage visit, per recipient, per ISO week (Budapest). The job runs once per Budapest day, at or after 07:00. | backend/src/service/automation.rs:~118-135; docs/DECISIONS.md:89-90; backend/src/jobs/mod.rs:~212-219 | documented |

### Blocker load (`/reports/blocker-load`)
| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| REP-62 | A blocker counts if it was created within the period. Blockers are grouped by who is responsible: the partner name if set, else the responsible email, else "(nincs felelős)". | backend/src/repo/reports.rs:243-253 | documented |
| REP-63 | For each group the report gives: total blockers; open (unresolved); overdue (unresolved and due date before Budapest today); nudges sent; and waiting days, the sum of (resolved time, or now) minus created time. Rows are sorted by waiting days, highest first. | backend/src/repo/reports.rs:224-233,243-254; backend/src/api/reports.rs:227 | documented |
| REP-64 | `share_of_waiting` is the row's waiting days divided by the total. When the total is 0, every share is 0. `total_waiting_days` is that total. | backend/src/api/reports.rs:204,229-240 | documented |

### FX rates (`/reports/fx-rates`)
| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| REP-66 | Returns the stored MNB rates for `base` within the period, both days included. `base` defaults to EUR and is converted to upper case. A rate is the number of HUF per 1 unit of the base currency. | backend/src/api/reports.rs:250-269; 0006_reporting.sql:4-13 | documented |
| REP-67 | There is at most one rate per (day, base, quote), and every rate is greater than 0. | 0006_reporting.sql:9,12 | documented |
| REP-68 | The background worker fetches MNB rates once per Budapest day, at or after 12:30, covering the last 10 days. | backend/src/jobs/mod.rs:~221-229; README.md:59 | documented |

### Web dashboard
| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| REP-70 | There are four stat tiles, each a link. Cars in work = open orders (links to /orders). Blockers = open blockers (links to /orders). Stalled = rows in `/reports/stalled` (links to /reports). Quotes = open leads whose quote expires within 7 days (links to /leads). Each tile shows "—" until its own data has loaded. | frontend/src/components/dashboard/DashboardView.tsx:3-5,119-142,155-162 | documented |
| REP-71 | Every tile number is a link into the list that owns it, so the list it opens must show the same set of records. | DashboardView.tsx:4 | documented |
| REP-72 | Expiring quotes are open leads with `quote_valid_until` on or before Budapest today + 7 days. Quotes that have already expired are included. | DashboardView.tsx:102-104; DateQuickPicks.tsx:6-11 | documented |
| REP-73 | The "in work" list puts orders that have a due date first (earliest due first), then orders by how long they have been in their current stage (longest first). | DashboardView.tsx:5,89-96 | documented |
| REP-74 | The attention section lists open blockers (overdue first, then by due date, with no due date last), then stalled orders, then expiring quotes. Its count is the sum of all three. | DashboardView.tsx:97-100,215-285 | documented |
| REP-75 | The pipeline shows at most 10 open leads, sorted by quote validity (earliest first), with leads that have no validity date last. | DashboardView.tsx:105-107 | documented |
| REP-76 | In "my tasks", overdue tasks (due date before Budapest today) come first and carry an overdue badge; the rest are sorted by due date. Each task links to the order, lead or partner it belongs to. | DashboardView.tsx:24-29,108-114,330-337 | documented |
| REP-77 | "Ready" lists up to 30 orders in the `completed` stage, most recently completed first, each with a "ready since" date. | DashboardView.tsx:81-85,115-117,356-389 | documented |
| REP-78 | Each section loads and fails on its own, with its own loading skeleton and its own error-and-retry. One slow or failed request does not blank the page. | DashboardView.tsx:50-53,119-120 | documented |
| REP-79 | An overdue blocker in the attention list should carry a badge that says it is overdue. It is driven by `is_overdue`. | DashboardView.tsx:237; backend/src/repo/blockers.rs:51 | implied |

### Time rules
| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| TIME-01 | "Today" everywhere (server defaults, overdue checks, report windows, date quick-picks, dashboard) is the Budapest calendar date, never UTC and never the device's time zone. | backend/src/service/mod.rs:15-17; backend/src/config.rs:458; WorkloadCharts.tsx:46-48; Format.kt:11 | documented |
| TIME-02 | The business time zone comes from `BUSINESS_TIMEZONE` (default `Europe/Budapest`). All server calendar logic uses this one setting. | backend/src/config.rs:177,458 | documented |
| TIME-03 | A blocker is overdue when it is unresolved, has a due date, and that date is before today. A blocker due today is not overdue yet. | backend/src/repo/blockers.rs:51,74,100; backend/src/domain/blocker.rs:39-41 | documented |
| TIME-04 | A task is overdue when its due date is before Budapest today. | DashboardView.tsx:108-111,331 | implied |
| TIME-05 | `days_overdue` = today − due date, in whole days. A nudge is sent only when today is after the due date, and not within `nudge_interval_days` × 24 hours of the previous nudge. | backend/src/domain/blocker.rs:39-50; backend/src/service/email.rs:235-237 | documented |
| TIME-06 | "Days in stage" is whole elapsed 24-hour periods (rounded down), never below 0, and computed the same way on the web, on Android and in the stalled report. | frontend/src/lib/utils/format.ts:79-83; Format.kt:55-60; backend/src/repo/reports.rs:208 | documented |
| TIME-07 | Timestamps are shown in Budapest local time. Plain dates (due date, valuation date, quote validity) are calendar dates and never shift with time zones. | Format.kt:36-53; DateQuickPicks.tsx:6-11 | implied |
| TIME-08 | The date quick-picks "Ma", "+7" and "+30" give Budapest today plus N calendar days. | DateQuickPicks.tsx:6-20 | documented |
| TIME-09 | Stage history uses `clock_timestamp()`, so the history order matches the order in which changes were committed. | docs/DECISIONS.md:44-45 | documented |
| TIME-10 | Scheduler timing: nudges once per Budapest hour; stalled alerts once per Budapest day after 07:00; FX fetch once per Budapest day after 12:30. | backend/src/jobs/mod.rs:~197-229 | documented |
| TIME-11 | A MiniCRM timestamp with no zone is read as Budapest local time. An imported order's valuation date is the Budapest date of its `CreatedAt`. If `CreatedAt` cannot be parsed, the valuation date is today and a problem is logged. | backend/src/migration/load.rs:308-321,879,925-931 | documented |

### Pagination
| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| PAGE-01 | Lists return `{"items": [...]}`. `limit` defaults to 50 and is clamped to 1–200. `offset` defaults to 0, and negative values become 0. | docs/API.md:24; backend/src/api/mod.rs:137-143 | documented |
| PAGE-02 | On the web, page = floor(offset / limit) + 1. "Previous" is disabled at offset 0. The label shows "offset+1 – offset+loaded", or an empty message when nothing loaded. | frontend/src/components/ui/Pagination.tsx:25,40,43 | documented |
| PAGE-03 | Jump-to-page accepts only a positive whole number and strips non-digits. The target page is not checked against a total (see F3). | Pagination.tsx:20,29-35,56 | documented |
| PAGE-04 | With the same sort and unchanged data, each record appears on exactly one page. No record repeats or goes missing between page N and N+1. | backend/src/repo/mod.rs:54-58 | assumed |
| PAGE-05 | An unknown `sort` key is rejected; a blank one falls back to the list's default. A leading `-` means descending. | backend/src/repo/mod.rs:54-66,93-99 | documented |
| PAGE-06 | Each user's page size is 25, 50 or 100 (default 50) and is used as `limit` on web lists. Any other value gets 400 "page_size: expected 25, 50 or 100". | backend/migrations/0008_user_settings.sql:3,7; backend/src/api/auth.rs:303-306; frontend/src/app/[locale]/leads/page.tsx:65-71,95 | documented |

### Global search
| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| SRCH-01 | `GET /search` is open to every signed-in user. | backend/src/api/search.rs:6-7,50 | documented |
| SRCH-02 | The query is trimmed. Under 2 characters (counted as Unicode characters) returns 200 with three empty groups, not an error. | backend/src/api/search.rs:31-32,53-60 | documented |
| SRCH-03 | Orders match on number, title, partner name, the order's own (legacy) VIN, a linked vehicle's VIN, or the plate. Plates are compared with only letters and digits, upper-cased, so "abc123" finds "ABC-123". | backend/src/repo/search.rs:64-71; backend/src/domain/order.rs:26-32; backend/tests/search.rs:160-175 | documented |
| SRCH-04 | Partners match on name, city, tax number, or phone. Phone formats are unified: "+36 30 123 4567", "06-30-123-4567", "0036301234567", "301234567" and "30 123 4567" all find the same partner. | backend/src/repo/search.rs:93-96; backend/src/repo/mod.rs:46-52; backend/tests/search.rs:69-100 | documented |
| SRCH-05 | Leads match on title, contact name, contact email, partner name, or contact phone (unified the same way). | backend/src/repo/search.rs:121-124 | documented |
| SRCH-06 | Text matching is a case-insensitive "contains" match. `%`, `_` and `\` in the query are matched literally, not as wildcards. | backend/src/repo/mod.rs:34-44,77-81 | documented |
| SRCH-07 | A query with no digits adds no phone condition, so a query that matches no text returns nothing. | backend/tests/search.rs:136-157 | documented |
| SRCH-08 | Each group returns at most 8 hits, newest first (highest id). The server does not rank across groups. | backend/src/api/search.rs:5-6,25-26; backend/src/repo/search.rs:8,72,97,125 | documented |
| SRCH-09 | An order hit shows the number, title, current stage, and a plate: the first linked vehicle's plate in alphabetical order, or else the legacy plate. A partner hit shows name, kind and city. A lead hit shows title, contact and stage. | backend/src/repo/search.rs:17-40,53-58 | documented |
| SRCH-10 | The web header search waits 250 ms after typing, searches only at 2+ characters, and shows a "min chars" hint below that. Groups always appear in the order orders → partners → leads. Arrow keys wrap around, Enter opens the highlighted hit, `/` focuses the box (unless you are typing somewhere), Esc closes it, and moving to another page clears it. | GlobalSearch.tsx:3-6,30-31,37-48,55-69,93-102,131-155,220-221 | documented |
| SRCH-11 | Recent searches (at most 6, shared by the header box and the Ctrl/⌘+K palette) are kept only in the browser, and a search is saved only when a hit is opened. | GlobalSearch.tsx:5-6,32-35,104-110; CommandPalette.tsx:5,63 | documented |
| SRCH-12 | The palette offers "new order/lead/partner" only to users allowed to create that kind of record. | CommandPalette.tsx:99-107 | documented |
| SRCH-13 | Case-insensitive matching also covers Hungarian accented letters ("ő" finds "Ő", "á" finds "Á"). | backend/src/repo/mod.rs:78 (test uses "müller") | assumed |

### Settings, admin and audit
| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| CFG-01 | Any signed-in user can read `GET /settings`. Only a user with `ManageSettings` (admin) can `PUT`; anyone else gets 403. | backend/src/api/configuration.rs:402,560; docs/API.md:30-32 | documented |
| CFG-02 | Settings is a single row. It starts with automatic email off, at most 3 automatic emails per recipient per day, a send window of 08:00–17:00 on weekdays only, a nudge every 3 days, escalation after 2 nudges, stage notifications off, and no stalled-alert recipients. | backend/migrations/0001_foundation.sql:62-79; docs/DECISIONS.md:91-92 | documented |
| CFG-03 | Saving fails with 400 if the send-window start is not before its end, if the daily email cap is below 0, if the nudge interval or escalation count is below 1, or if any stalled-alert recipient is not a valid email address. | configuration.rs:561-578; 0001_foundation.sql:68-73 | documented |
| CFG-04 | Transport fields follow PATCH rules: a missing field keeps the stored value, `null` resets it to the environment, and blank strings count as missing. `mode: null` resets the whole transport. SMTP needs a host and a port from 1 to 65535. | configuration.rs:406-410,483-527,665-700 | documented |
| CFG-05 | The SMTP password is never returned; reads show only `has_password`. | backend/src/repo/config.rs:227,287 | documented |
| CFG-06 | Every settings save writes an audit entry (entity `settings`, action `update`) with before and after, in the same transaction. Turning the kill switch on or off is also logged as a warning. | configuration.rs:612-627; backend/src/repo/audit.rs:6-7 | documented |
| CFG-07 | Stage keys are permanent. They must be at most 50 characters of lowercase letters, digits and underscores, starting with a letter. An exit stage must also be terminal. `stall_after_days` must be greater than 0. `min_images` must be 0 or more, and if it is above 0 an image category is required. | configuration.rs:37-42,84-101,114-122; docs/API.md:158-159 | documented |
| CFG-08 | The lead stage "won" cannot be deactivated. At least one active, non-terminal stage must remain for each entity. Breaking either rule gets 422 `stage_required`. | configuration.rs:198-222 | documented |
| CFG-09 | Changing configuration needs `ManageConfiguration` (admin). `spec_form` on a project type must be `heating`, `cooling` or absent. | configuration.rs:113,291,330-336,352 | documented |
| CFG-10 | Each user's preferences are density (`comfortable` or `compact`) and page size (25, 50 or 100). A user who never saved any gets the defaults. Preferences never change how the system behaves. | 0008_user_settings.sql:1-7; backend/src/api/auth.rs:254-275,294-307 | documented |
| CFG-11 | Every admin endpoint (status, jobs, retry, run, fx/fetch, email/test) needs `OperateSystem`. | backend/src/api/admin.rs:55,96,139,159,186,246 | documented |
| CFG-12 | `POST /admin/run/{kind}` accepts only `nudge_blockers` or `stalled_orders`; anything else gets 400. It queues the job and returns 202 with the job id. | admin.rs:44-73; docs/API.md:185 | documented |
| CFG-13 | Only failed (dead-lettered) jobs can be retried. Retrying any other job gets 409. `GET /admin/jobs?state=` accepts `failed` (the default) or `pending`. | admin.rs:124,144,160-165 | documented |
| CFG-14 | `POST /admin/fx/fetch` returns 400 if `from > to`. Asking for the same manual range again does not queue a second job. | admin.rs:187-197 | documented |
| CFG-15 | `/admin/status` shows the transport that is actually in effect (a database override wins over the environment), plus failed and pending job counts, emails needing attention, orders missing an FX rate, and the date of the latest EUR rate. | admin.rs:97-120; docs/API.md:181 | documented |
| CFG-16 | The admin email test always returns 200. A delivery failure comes back as a result with a reason. A candidate transport is checked but never saved. In `dry_run` mode nothing leaves the machine. | admin.rs:202-235 | documented |
| CFG-17 | An audit entry is written in the same transaction as the change it records, with the user (null for system actions), the entity, the id, the action, and a diff of only the fields that changed (`{field: [old, new]}`). | backend/src/repo/audit.rs:6-27,64-77; 0001_foundation.sql:51-59 | documented |
| CFG-18 | Audit history is listed newest first (by time, then id), limited by `limit`, with each user's display name. | backend/src/repo/audit.rs:42-58 | documented |

### Import / MiniCRM migration
| ID | Expected behavior | Source (file:line) | Confidence |
|---|---|---|---|
| IMP-01 | The pipeline runs extract → manifest → fetch → load → reconcile. Extract never transforms data. Extract and fetch can be resumed, and fetch skips files it already has. | backend/src/migration/mod.rs:1-9; docs/DECISIONS.md:114; docs/migration/README.md:4,35 | documented |
| IMP-02 | Load is idempotent: rows are upserted by `minicrm_id`. Running it again creates no duplicates, no extra images and no extra stage rows. | load.rs:1-6; docs/DECISIONS.md:124; tests/migration.rs:188-201 | documented |
| IMP-03 | `--dry-run` resolves every field exactly as a real load would but writes nothing. | load.rs:366-368; tests/migration.rs:141-152 | documented |
| IMP-04 | If a project type, assignee or other value in the source is not covered by the mapping, the load stops before writing any row and lists every gap at once. | load.rs:8-13,76-80,400-402; tests/migration.rs:330 | documented |
| IMP-05 | The full source record is kept in `raw_import`. `GET /{partners,leads,orders}/{id}/raw-import` is open to any signed-in user and returns nulls (not 404) for records created in AutoCRM. It returns 404 only if the record itself does not exist. | backend/src/api/raw_import.rs:1-8,30-55; docs/DECISIONS.md:124-125 | documented |
| IMP-06 | An imported order gets exactly one stage row: its final stage, dated when it entered that stage (`StatusUpdatedAt`, else `CreatedAt`). Stage-duration and throughput reports are meaningful only for orders created in AutoCRM. | docs/migration/README.md:132-146; load.rs:880-882 | documented |
| IMP-07 | An imported order's value becomes one line item. Its currency comes from the mapped field when that is HUF or EUR, else the mapping's default currency. | load.rs:62-65,920-924 | documented |
| IMP-08 | MNB rates must be backfilled for the historical period after load, so that imported EUR orders convert to HUF instead of counting as `missing_fx`. | docs/migration/README.md:112; docs/history/VIABILITY.md:47 | documented |
| IMP-09 | Imported intake photos get object-lock retention at load time. Originals are kept for every photo category. | docs/DECISIONS.md:119-123; tests/migration.rs:248-250 | documented |
| IMP-10 | Reconciliation writes a Markdown report with a sign-off section. It compares record counts (marking mismatches with ⚠️), lists missing orders by MiniCRM id, and covers file coverage, file counts per order, value by year, per-column coverage, and a random field-level sample checked against the source. | backend/src/migration/reconcile.rs:1-3,299-304; docs/DECISIONS.md:130-131; tests/migration.rs:291-316 | documented |
| IMP-11 | MiniCRM to-dos become append-only `order_notes`, kept in their original order. To-dos on leads or skipped categories are dropped. | load.rs:1087-1096,1136; tests/migration.rs:272-282 | documented |
