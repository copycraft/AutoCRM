# AutoCRM showcase

For an occasional showing from the home Ubuntu server. Everything runs in Docker; mail is
only logged and NAV invoicing is off. The stack publishes two ports for your tunnel:

| Port (host) | What | Public hostname |
|---|---|---|
| `APP_PORT` (3000) | Web app (also serves `/api`) | `APP_HOST`, e.g. `crm-demo.example.com` |
| `FILES_PORT` (9000) | File storage (photo links) | `FILES_HOST`, e.g. `files.crm-demo.example.com` |

Both must be reachable over **https** at those hostnames: login cookies are Secure, and
photo links are signed for `FILES_HOST`. Keep the tunnel passing the original `Host`
header for the files hostname.

## Once

1. Docker:
   ```bash
   sudo apt-get install -y docker.io docker-compose-v2 && sudo usermod -aG docker $USER
   ```
   (log out and back in)
2. `cp .env.example .env` and fill it in (secrets: `openssl rand -hex 32`). Set `BIND_IP=0.0.0.0`
   if your tunnel runs on another machine.
3. Point your tunnel at `APP_PORT` and `FILES_PORT`.

No tunnel of your own? Leave the rest as is, set `TUNNEL_TOKEN` from a Cloudflare tunnel
(hostnames → `frontend:3000` and `minio:9000`), and `setup.sh` starts cloudflared too.

## Each showing

```bash
./setup.sh --build   # --build only the first time or after pulling new code
```

Sign in at `https://APP_HOST` as `admin@demo.local`, `iroda@demo.local`,
`tervezo@demo.local` or `nezo@demo.local`, all with `DEMO_PASSWORD`.

Afterwards: `docker compose down`.
