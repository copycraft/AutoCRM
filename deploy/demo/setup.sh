#!/usr/bin/env bash
# Starts the showcase from scratch: wipes the database and files, migrates, and creates
# one demo login per role (password DEMO_PASSWORD). Run it again before a showing to
# start clean.
#
#   ./setup.sh            start fresh
#   ./setup.sh --build    rebuild the images first (after pulling new code)
#   docker compose down   stop it after the showing
#
# With NFS_SERVER set in .env the files go to your NAS share (docker-compose.nfs.yml).
# Files already on the share are never deleted: point a demo at an empty export.
set -euo pipefail
cd "$(dirname "$0")"
set -a; . ./.env; set +a
: "${DEMO_PASSWORD:?set DEMO_PASSWORD in .env}"

compose=(docker compose)
if [ -n "${NFS_SERVER:-}" ]; then
  compose+=(-f docker-compose.yml -f docker-compose.nfs.yml)
fi

if [ "${1:-}" = "--build" ]; then "${compose[@]}" build; fi

"${compose[@]}" --profile cloudflared down --volumes --remove-orphans
if [ -n "${TUNNEL_TOKEN:-}" ]; then "${compose[@]}" --profile cloudflared up -d; else "${compose[@]}" up -d; fi


"${compose[@]}" run --rm --no-deps -e AUTOCRM_ADMIN_PASSWORD="$DEMO_PASSWORD" migrate \
  autocrm create-admin --email admin@demo.local --name "Demo Admin"

# The other roles share the admin's password hash; nobody is asked to change it.
"${compose[@]}" exec -T postgres psql -U autocrm -d autocrm -v ON_ERROR_STOP=1 <<'SQL'
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


echo "demo ready: https://${APP_HOST}"
if [ -n "${NFS_SERVER:-}" ]; then echo "files: ${NFS_SERVER}:${NFS_EXPORT} (projektek/, hr/, ...)"; fi
echo "showcase data (leads, orders, walkarounds, staff): node seed.mjs"
