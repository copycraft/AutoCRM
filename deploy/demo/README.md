# AutoCRM showcase

For an occasional showing from the home Ubuntu server. Everything runs in Docker; a
Cloudflare Tunnel makes it reachable without opening ports. Mail is only logged and NAV
invoicing is off.

## Once

1. Docker:
   ```bash
   sudo apt-get install -y docker.io docker-compose-v2 && sudo usermod -aG docker $USER
   ```
   (log out and back in)
2. Cloudflare → **Zero Trust → Networks → Tunnels → Create a tunnel**, copy the token, and
   add two public hostnames:
   - `crm-demo.<domain>` → HTTP `frontend:3000`
   - `files.crm-demo.<domain>` → HTTP `minio:9000`
3. `cp .env.example .env` and fill it in (secrets: `openssl rand -hex 32`).

## Each showing

```bash
./setup.sh --build   # --build only the first time or after pulling new code
```

Sign in at `https://crm-demo.<domain>` as `admin@demo.local`, `iroda@demo.local`,
`tervezo@demo.local` or `nezo@demo.local`, all with `DEMO_PASSWORD`.

Afterwards: `docker compose down`.
