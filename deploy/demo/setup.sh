#!/usr/bin/env bash
# Starts the showcase from scratch: wipes the database and files, migrates, and creates
# one demo login per role (password DEMO_PASSWORD). Run it again before a showing to
# start clean.
#
#   ./setup.sh            start fresh
#   ./setup.sh --build    rebuild the images first (after pulling new code)
#   docker compose down   stop it after the showing
set -euo pipefail
cd "$(dirname "$0")"
set -a; . ./.env; set +a
: "${DEMO_PASSWORD:?set DEMO_PASSWORD in .env}"

if [ "${1:-}" = "--build" ]; then docker compose build; fi

docker compose --profile cloudflared down --volumes --remove-orphans
if [ -n "${TUNNEL_TOKEN:-}" ]; then docker compose --profile cloudflared up -d; else docker compose up -d; fi


docker compose run --rm --no-deps -e AUTOCRM_ADMIN_PASSWORD="$DEMO_PASSWORD" migrate \
  autocrm create-admin --email admin@demo.local --name "Demo Admin"

# The other roles share the admin's password hash; nobody is asked to change it.
docker compose exec -T postgres psql -U autocrm -d autocrm -v ON_ERROR_STOP=1 <<'SQL'
INSERT INTO users (email, display_name, role, password_hash, must_change_password)
SELECT v.email, v.name, v.role::user_role, u.password_hash, false
FROM users u,
     (VALUES ('iroda@demo.local', 'Iroda Ilona', 'office'),
             ('tervezo@demo.local', 'Tervező Tamás', 'designer'),
             ('nezo@demo.local', 'Néző Nóra', 'viewer')) AS v(email, name, role)
WHERE u.email = 'admin@demo.local';
UPDATE users SET must_change_password = false;
-- Show the per-user grants off: the viewer may comment.
UPDATE users SET permissions = '{comment}' WHERE email = 'nezo@demo.local';
SQL


# Something to show: leads, orders, finished walkarounds, employees with days off. Goes
# through the API as the demo admin (signature uploads use the public FILES_HOST link, so
# the tunnel must be up). A refusal is printed but does not stop the setup; --no-seed skips it.
if [ "${1:-}" != "--no-seed" ] && [ "${2:-}" != "--no-seed" ]; then
  until curl -fsS "http://127.0.0.1:${APP_PORT:-3000}/api/health" >/dev/null 2>&1 \
     || curl -fsS "http://127.0.0.1:${APP_PORT:-3000}/hu/login" >/dev/null 2>&1; do sleep 2; done
  docker run --rm --network host -v "$PWD/seed.mjs:/seed.mjs:ro" \
    -e BASE_URL="http://127.0.0.1:${APP_PORT:-3000}" -e DEMO_PASSWORD="$DEMO_PASSWORD" \
    node:20-alpine node /seed.mjs || echo "seed finished with problems (see above); the demo still works"
fi

echo "demo ready: https://${APP_HOST}"
