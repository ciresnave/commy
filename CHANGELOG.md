# Changelog

## 0.5.0 (breaking)
- `CreateService` requires `size` (bytes, a positive integer). The file is rounded up to whole 4 KiB pages and may not exceed the server maximum (default 10 MiB; `COMMY_MAX_SERVICE_SIZE` until config-file support exists). A missing, zero, negative or non-integer size is refused as `InvalidRequest`. The `Service` reply carries the final `size`.
- `Tenant::register_service` takes a `size` argument. `Server::max_service_size`, `Server::with_max_service_size`, `service_file_size`, `ServiceSizeError`, `PAGE_SIZE`, `DEFAULT_MAX_SERVICE_SIZE` are new.

## 0.4.1
- Untrack committed example build output (fixes "filename too long" on Windows clones). This is the release that ships the 0.4.0 security fix.

## 0.4.0 (breaking, security)
- Every operation except `Heartbeat`/`Disconnect` needs an authenticated session; `Authenticate` is refused unless `COMMY_ALLOW_DEV_AUTH=1`; tenant ids are validated (`[A-Za-z0-9_-]{1,64}`) before use in paths; `DeleteTenant` removes the tenant; services are checked against the session's tenant. GHSA-5q28-5hrh-w432, GHSA-rwg3-grc8-336p.
