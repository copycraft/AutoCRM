#!/usr/bin/env bash
# Central build: everything needed to deploy AutoCRM, in ./build.
#
#   ./build.sh                 build everything
#   ./build.sh --skip android  skip a part (backend | frontend | sidecar | android), repeatable
#
# Environment:
#   AUTOCRM_API_URL      API origin the web frontend proxies /api to (baked in at build time).
#                        Default http://127.0.0.1:8080 (the API on the same host).
#   AUTOCRM_PUBLIC_URL   Address phones use; prefilled in the APK's setup screen.
#                        Default https://crm.autotherm.hu
#   AUTOCRM_S3_URL       Object-storage origin for presigned image URLs (frontend).
#   LINUX_BINARY=1       Also build a Linux x86_64 backend binary in Docker (needs a running
#                        Docker daemon). The README deploys on Linux; the host binary is for
#                        whatever machine runs this script.
#   JAVA_HOME            JDK 17+ for the Android build. Falls back to ~/android-tools/jdk.
#
# Needs: cargo, node + npm, a JDK and the Android SDK (android/local.properties).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="$ROOT/build"
SKIP=" "
while [ $# -gt 0 ]; do
  case "$1" in
    --skip) SKIP="$SKIP$2 "; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
skipped() { [[ "$SKIP" == *" $1 "* ]]; }
step() { printf '\n\033[1m==> %s\033[0m\n' "$*"; }

API_URL="${AUTOCRM_API_URL:-http://127.0.0.1:8080}"
PUBLIC_URL="${AUTOCRM_PUBLIC_URL:-https://crm.autotherm.hu}"
EXE=""; case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) EXE=".exe" ;; esac

# Selective builds keep the parts they do not touch.
if [ "$SKIP" = " " ]; then rm -rf "$OUT"; fi
mkdir -p "$OUT"

# --- backend -------------------------------------------------------------------------------
if ! skipped backend; then
  step "Backend (release binary; queries checked against the committed .sqlx metadata)"
  rm -rf "$OUT/backend"; mkdir -p "$OUT/backend"
  (cd "$ROOT/backend" && SQLX_OFFLINE=true cargo build --release --bins)
  cp "$ROOT/backend/target/release/autocrm$EXE" "$OUT/backend/"
  [ -f "$ROOT/backend/target/release/autocrm-migrate$EXE" ] &&
    cp "$ROOT/backend/target/release/autocrm-migrate$EXE" "$OUT/backend/"
  cp "$ROOT/backend/.env.example" "$OUT/backend/.env.example"
  # Migrations are embedded in the binary (`autocrm migrate`); the SQL is kept for reference.
  cp -r "$ROOT/backend/migrations" "$OUT/backend/migrations"

  if [ "${LINUX_BINARY:-0}" = "1" ]; then
    step "Backend (Linux x86_64, in Docker)"
    mkdir -p "$OUT/backend/linux-x86_64"
    docker run --rm -v "$ROOT/backend:/src" -w /src -e SQLX_OFFLINE=true \
      -e CARGO_TARGET_DIR=/src/target-linux rust:1-bookworm \
      cargo build --release --bins
    cp "$ROOT/backend/target-linux/release/autocrm" "$OUT/backend/linux-x86_64/"
    [ -f "$ROOT/backend/target-linux/release/autocrm-migrate" ] &&
      cp "$ROOT/backend/target-linux/release/autocrm-migrate" "$OUT/backend/linux-x86_64/"
  fi
fi

# --- frontend ------------------------------------------------------------------------------
if ! skipped frontend; then
  step "Frontend (Next.js standalone server, /api proxied to $API_URL)"
  rm -rf "$OUT/frontend"; mkdir -p "$OUT/frontend"
  (
    cd "$ROOT/frontend"
    npm ci --no-audit --no-fund
    export AUTOCRM_API_URL="$API_URL"
    [ -z "${AUTOCRM_S3_URL:-}" ] || export AUTOCRM_S3_URL
    npm run build
  )
  STANDALONE="$ROOT/frontend/.next/standalone"
  # In a monorepo layout Next nests the app; this repo's frontend is the tracing root.
  APPDIR="$STANDALONE"
  [ -f "$STANDALONE/server.js" ] || APPDIR="$(dirname "$(find "$STANDALONE" -name server.js -not -path '*/node_modules/*' | head -1)")"
  cp -r "$APPDIR/." "$OUT/frontend/"
  mkdir -p "$OUT/frontend/.next"
  cp -r "$ROOT/frontend/.next/static" "$OUT/frontend/.next/static"
  [ -d "$ROOT/frontend/public" ] && cp -r "$ROOT/frontend/public" "$OUT/frontend/public"
  cp "$ROOT/frontend/.env.example" "$OUT/frontend/.env.example"
fi

# --- NAV sidecar ---------------------------------------------------------------------------
if ! skipped sidecar; then
  step "NAV sidecar (compiled JS + production dependencies)"
  rm -rf "$OUT/nav-sidecar"; mkdir -p "$OUT/nav-sidecar"
  (cd "$ROOT/nav-sidecar" && npm ci --no-audit --no-fund && npm run build)
  cp -r "$ROOT/nav-sidecar/dist" "$OUT/nav-sidecar/dist"
  cp "$ROOT/nav-sidecar/package.json" "$ROOT/nav-sidecar/package-lock.json" "$OUT/nav-sidecar/"
  cp "$ROOT/nav-sidecar/Dockerfile" "$ROOT/nav-sidecar/README.md" "$OUT/nav-sidecar/"
  (cd "$OUT/nav-sidecar" && npm ci --omit=dev --no-audit --no-fund)
fi

# --- Android -------------------------------------------------------------------------------
if ! skipped android; then
  step "Android (signed release APK)"
  rm -rf "$OUT/android"; mkdir -p "$OUT/android"
  if [ -z "${JAVA_HOME:-}" ] && [ -d "$HOME/android-tools/jdk" ]; then
    export JAVA_HOME="$HOME/android-tools/jdk"
  fi
  KEYTOOL="${JAVA_HOME:+$JAVA_HOME/bin/}keytool"
  # One signing key for the life of the app: an update is only accepted over the same key.
  if [ ! -f "$ROOT/android/keystore.properties" ]; then
    echo "No android/keystore.properties: generating a release keystore. BACK IT UP."
    PASS="$(head -c 24 /dev/urandom | od -An -tx1 | tr -d ' \n')"
    "$KEYTOOL" -genkeypair -v -keystore "$ROOT/android/autocrm-release.keystore" \
      -alias autocrm -keyalg RSA -keysize 4096 -validity 10000 \
      -storepass "$PASS" -keypass "$PASS" -dname "CN=AutoCRM, O=Autotherm, C=HU" >/dev/null
    printf 'storeFile=autocrm-release.keystore\nstorePassword=%s\nkeyAlias=autocrm\nkeyPassword=%s\n' \
      "$PASS" "$PASS" > "$ROOT/android/keystore.properties"
  fi
  (
    cd "$ROOT/android"
    AUTOCRM_API_URL="$PUBLIC_URL" ./gradlew --no-daemon assembleRelease
  )
  APK="$(find "$ROOT/android/app/build/outputs/apk/release" -name '*.apk' | head -1)"
  [ -n "$APK" ] || { echo "release APK not found" >&2; exit 1; }
  cp "$APK" "$OUT/android/autocrm.apk"
fi

# --- reference material --------------------------------------------------------------------
step "Docs, API contract, deploy notes"
rm -rf "$OUT/docs" "$OUT/openapi"
mkdir -p "$OUT/docs" "$OUT/openapi"
cp "$ROOT/openapi/openapi.json" "$OUT/openapi/"
cp "$ROOT/README.md" "$ROOT/docker-compose.yml" "$OUT/"
cp "$ROOT/docs/websiteleadsinstructions.md" "$ROOT/docs/API.md" "$ROOT/docs/error-codes.md" \
   "$ROOT/docs/email-google-workspace.md" "$OUT/docs/"

cat > "$OUT/DEPLOY.md" <<EOF
# Deploying this build

Built $(date -u +%Y-%m-%dT%H:%M:%SZ) from commit $(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown).

| Folder | What it is | How to run |
|---|---|---|
| \`backend/\` | The API: \`autocrm\`$EXE (+ \`autocrm-migrate\`$EXE, \`.env.example\`, \`migrations/\`) | Copy \`.env.example\` to \`.env\` and fill it in. \`./autocrm migrate\`, \`./autocrm create-admin\`, then \`./autocrm serve\` |
| \`backend/linux-x86_64/\` | Linux server binary (only with \`LINUX_BINARY=1\`) | Same commands |
| \`frontend/\` | Web app, Next.js standalone server | \`PORT=3000 HOSTNAME=127.0.0.1 node server.js\` (Node 20+). \`/api\` is proxied to \`$API_URL\` (fixed at build time) |
| \`nav-sidecar/\` | NAV Online Számla sidecar | \`node dist/index.js\`, or build the Docker image from its \`Dockerfile\`. Needs \`SIDECAR_TOKEN\` and the NAV technical user (see its README) |
| \`android/autocrm.apk\` | Signed release APK, prefilled server \`$PUBLIC_URL\` | Install on the phones; the server can be changed on first run |
| \`openapi/openapi.json\` | The API contract | |
| \`docs/websiteleadsinstructions.md\` | Send this to the website developer | |

Also needed on the server: PostgreSQL 16 and an S3-compatible object store (see \`docker-compose.yml\`
and \`README.md\`). Set \`LEADS_API_KEY\` to enable the website lead endpoint
(\`openssl rand -hex 32\`), and \`NEWSLETTER_API_KEY\` for newsletter signup.
EOF

step "Done. Contents of $OUT:"
(cd "$OUT" && find . -maxdepth 2 -not -path './frontend/*' -not -path './nav-sidecar/*' -not -path './backend/migrations/*' | sort)
