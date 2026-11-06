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
(hostnames → `frontend:3000` and `s3:9000`), and `setup.sh` starts cloudflared too.

## Files on your NAS (optional)

Instead of the bundled object store, the backend can write every file as a plain file in
a project folder on an NFS share, so you can browse it from the NAS:

```text
projektek/2026/2026-0042_Kovacs-Kft_ABC-123/
    ajanlat/  dokumentumok/<terv|cad|...>/  szamlazas/<szamlak|sztornok|dijbekerok>/
    fotok/<bevetel|gyartas|...>/  atadas-atvetel/<nap>_<atvetel|kiadas>_<rendszam>_<id>/
ajanlatok/2026/L000123_.../   hr/dolgozok/...   hr/toborzas/...   bejovo-szamlak/2026/10/
```

1. On the NAS: export a folder over NFS to this server (read-write), owned by the user
   the backend should write as.
2. Here: `sudo apt-get install -y nfs-common`
3. In `.env`: `NFS_SERVER`, `NFS_EXPORT`, and `FILES_UID`/`FILES_GID` (that owner's ids).

`setup.sh` then mounts the share through Docker; if it cannot, the backend does not start
rather than writing elsewhere. `FILES_HOST` is no longer needed: uploads and photo links go
through `APP_HOST`. Files already on the share are never deleted, so give a demo its own
empty export.

Switching a running stack that already has files in the object store (without `setup.sh`,
which would wipe them): bring it up with both compose files, then copy every file into its
folder on the share once; the object store is left as it was.

```bash
docker compose -f docker-compose.yml -f docker-compose.nfs.yml run --rm -e S3_ENDPOINT=http://s3:9000 backend autocrm storage-relayout --from-s3
```

## Each showing

```bash
./setup.sh --build   # --build only the first time or after pulling new code
```

Sign in at `https://APP_HOST` as `admin@demo.local`, `iroda@demo.local`,
`tervezo@demo.local` or `nezo@demo.local`, all with `DEMO_PASSWORD`.

Afterwards: `docker compose down`.
