# HTTPS with a reverse proxy, and backups

Written for: the person who runs the server and wants to reach it from another device, or keep a copy of its data.

Read [self-hosting](self-hosting.md) first. This guide adds two things: HTTPS in front of the server, and a backup routine you have tested.

## Why HTTPS

The server speaks plain HTTP. Your API key is sent in every request, so without HTTPS anyone on the same network can read it. Keep the server on `127.0.0.1` and put a proxy in front of it. The proxy takes the connection over HTTPS and passes it on to the server.

The Docker setup already listens only on `127.0.0.1:8080`, so the proxy runs on the same computer and is the only thing that can reach the server from outside.

## Caddy (recommended)

Caddy gets and renews its own certificate. Install it on the same machine, then create a `Caddyfile`:

```
ruc.example.com {
    reverse_proxy 127.0.0.1:8080
}
```

Replace `ruc.example.com` with a domain that points at your machine, and open ports 80 and 443 to it. Caddy uses those ports to obtain the certificate.

On a home network without a public domain, use Caddy's internal certificate authority instead. Your devices then need to trust that authority:

```
ruc.lan {
    tls internal
    reverse_proxy 127.0.0.1:8080
}
```

Then start Caddy with `caddy run` (or `caddy start`), and check it:

```sh
curl https://ruc.example.com/healthz
```

You should see `{"status":"ok"}`.

## Traefik (untested sketch)

Traefik can run in Docker next to the server. The example below is a sketch. It has not been run against a real Traefik install, so check the Traefik documentation for your version before you rely on it. It assumes Traefik and the server share a Docker network, and that Traefik is set up with a Let's Encrypt resolver named `letsencrypt`.

```yaml
services:
  tpt-ruc:
    # ...as in docker-compose.yml. Remove the "127.0.0.1:8080:8080" port mapping,
    # so only Traefik can reach the server.
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.tpt-ruc.rule=Host(`ruc.example.com`)"
      - "traefik.http.routers.tpt-ruc.entrypoints=websecure"
      - "traefik.http.routers.tpt-ruc.tls.certresolver=letsencrypt"
      - "traefik.http.services.tpt-ruc.loadbalancer.server.port=8080"
```

## What the proxy changes

- **Rate limits see the proxy, not the client.** The server limits requests by the address of the connection it receives. Behind a proxy, every client shares the proxy's address, so one busy client can slow everyone down. The server does not yet read the `X-Forwarded-For` header, so keep the number of users small, or do not use the proxy for many devices.
- **Logs show the proxy's address** as the client.
- **Browser apps on another origin need CORS.** If you serve the web app from a different address, add it to `TPT_RUC_CORS_ORIGINS`. If the server serves the web app itself (`TPT_RUC_PWA_DIR`), the proxy and the app share one origin, and no CORS setting is needed.

## Backups

Everything the server keeps is in one Docker volume: the database, which holds the trips, users, vehicles and devices, and the saved API key. A backup is a copy of that volume, taken while the server is stopped, so the database is not changing as it is copied.

### Find the volume name

Docker puts the project folder's name in front of the volume name, so the volume is usually called `tpt-ruc-nz_tpt-ruc-data`, not `tpt-ruc-data`. Find the exact name, then keep it in a shell variable for the commands below (bash syntax):

```sh
docker volume ls --filter name=tpt-ruc-data
VOLUME=tpt-ruc-nz_tpt-ruc-data   # use the name the command above printed
```

### Make a backup

```sh
docker compose down
docker run --rm -v "$VOLUME":/data -v "$PWD":/backup alpine \
  tar czf /backup/tpt-ruc-backup-$(date +%Y-%m-%d).tar.gz -C /data .
docker compose up -d
```

On Windows PowerShell, use `docker` with the same arguments, set `$VOLUME` with `$VOLUME = "..."`, and use a fixed file name instead of `$(date ...)`.

Store the archive somewhere private, away from the machine it came from. It contains your trips, and the API key, so treat it like the key itself.

### Test a restore

A backup you have never restored is not yet a backup. Test it on another machine, or in a separate Docker volume:

```sh
docker volume create tpt-ruc-restore-test
docker run --rm -v tpt-ruc-restore-test:/data -v "$PWD":/backup alpine \
  tar xzf /backup/tpt-ruc-backup-2026-10-01.tar.gz -C /data
```

Then start a second copy of the server on that volume, on a different port. The simplest way is a copy of `docker-compose.yml` in another folder, with its volume and port changed. Check that your trips are there:

```sh
curl -H "Authorization: Bearer YOUR-KEY" "http://localhost:8081/v1/trips?limit=5"
```

Remove the test volume when you have finished: `docker volume rm tpt-ruc-restore-test`.

### Restore for real

Stop the server, clear the volume, then unpack the archive into it:

```sh
docker compose down
docker run --rm -v "$VOLUME":/data alpine sh -c "rm -rf /data/* /data/.[!.]*"
docker run --rm -v "$VOLUME":/data -v "$PWD":/backup alpine \
  tar xzf /backup/tpt-ruc-backup-2026-10-01.tar.gz -C /data
docker compose up -d
```

### A logical copy, without stopping the server

`GET /v1/export` returns every trip you may see, as JSON. It does not include users, vehicles or the API key, so it is not a full backup. It is useful for moving trips to another system, or for keeping a readable copy of your records. Keep the volume backup for a full restore.

### How often

Back up after any change you would be sorry to lose, and at least once a month. Keep several old archives, so a damaged backup does not leave you with nothing.
