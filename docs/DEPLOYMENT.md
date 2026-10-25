# Running Commy

Commy runs as one server process. Clients connect over WebSocket Secure (WSS); clients on the same
machine can additionally map service files directly. There is no supported multi-server mode (see
`docs/CLUSTERING.md`).

## Configuration

Every setting can be an environment variable or a key in a TOML file (`COMMY_CONFIG`, else
`./commy.toml`; example: `commy.example.toml`). The environment overrides the file, which overrides
the default. An unknown key stops the start-up. Secrets and the development-authentication switch are
refused in the file.

| Variable (key) | Meaning | Default |
|---|---|---|
| `COMMY_LISTEN_ADDR` (`listen_addr`), `COMMY_LISTEN_PORT` (`listen_port`) | Where to listen | `0.0.0.0`, `8443` |
| `COMMY_TLS_CERT_PATH` (`tls_cert_path`), `COMMY_TLS_KEY_PATH` (`tls_key_path`) | PEM certificate chain and key (PKCS#8, PKCS#1 or SEC1). Both are needed for WSS. | none |
| `COMMY_STATE_DIR` (`state_dir`) | Durable state: service registry, grants, audit log. `off` keeps everything in memory. | `./commy_state` |
| `COMMY_AUTH_DIR` (`auth_dir`) | Per-tenant authentication databases (`<tenant>.sqlite`) | `commy_state/auth` |
| `COMMY_AUTH_MASTER_SECRET` | **Required** unless development authentication is on: at least 32 bytes, environment only. Per-tenant JWT secrets are derived from it. | none, no default |
| `COMMY_ALLOW_DEV_AUTH=1` | Accept any credential as administrator. Development only; environment only. | off |
| `COMMY_MAX_SERVICE_SIZE` (`max_service_size`) | Largest service file a client may request, in bytes | 10 MiB |
| `COMMY_DRAIN_TIMEOUT_MS`, `COMMY_FILL_TIMEOUT_MS` (`drain_timeout_ms`, `fill_timeout_ms`) | Resize timeouts | 5000, 30000 |
| `COMMY_AUDIT_MAX_EVENTS` (`audit_max_events`) | Audit events kept | 100000 |
| `COMMY_GRANT_COMPACT_AFTER` (`grant_compact_after`) | Stored grant changes that trigger folding the log into a snapshot; 0 = never | 1000 |
| `COMMY_ROTATION_MIN_INTERVAL_MS` (`rotation_min_interval_ms`) | Least time between two rotations of one service | 1000 |
| `COMMY_SERVER_ID` (`server_id`) | A name printed in the log | `node-1` |

`COMMY_CLUSTER_ENABLED` is read and only produces a warning: the server does not replicate
(`docs/CLUSTERING.md`).

## A minimal production start

```
export COMMY_AUTH_MASTER_SECRET="$(openssl rand -hex 32)"     # keep it; losing it invalidates JWTs
export COMMY_TLS_CERT_PATH=/etc/commy/server.pem
export COMMY_TLS_KEY_PATH=/etc/commy/server.key
commy
```

Then provision a tenant's users and API keys as `docs/AUTHENTICATION.md` describes; until a tenant's
database exists nobody can log in to it.

Service files are created under `tenant_<id>/` in the working directory. The tenant directory is
unlisted to other accounts (mode 0711 on Unix, an ACL on Windows) and file names are long and random. Run the
server under a dedicated account and keep its working directory off shared storage.

## Docker

The `Dockerfile` builds the server (`cargo build --release --locked`) and runs it as `commy` on port
8443 with `COMMY_STATE_DIR=/var/lib/commy`; mount a volume there. Pass the environment above with
`-e` or an env file, and mount the certificate and key read-only:

```
docker build -t commy .
docker run -d -p 8443:8443 -v commy-state:/var/lib/commy -v ./certs:/etc/commy:ro \
  -e COMMY_AUTH_MASTER_SECRET -e COMMY_TLS_CERT_PATH=/etc/commy/server.pem \
  -e COMMY_TLS_KEY_PATH=/etc/commy/server.key commy
```

The health check is a TCP connect to the listen port; the server has no HTTP health endpoint. (Each probe shows up in the log as a failed TLS handshake; that is expected.) The server also refuses to start without a certificate and key.
Zero-copy local mapping needs the clients on the same filesystem as the server, so it does not cross
a container boundary unless the tenant directory is a shared mount. Remote mirrors work anywhere.

`docker-compose.yml` is only a set of test databases for the future backend tests (ROADMAP R21; no test uses them
today); it does not start Commy.

## Operating notes

- **Restart:** services and grants come back; sessions, nonces and rotations in flight do not, and
  clients resynchronise.
- **Audit log:** `<state dir>/audit/`, best effort, hash-chained (`Server::verify_audit` detects modification, insertion and reordering, not truncation; record `Server::audit_head` off the machine to also catch a cut tail).
- **Backups:** the state directory plus the auth directory. Service contents are not persisted by
  Commy beyond the files themselves.
