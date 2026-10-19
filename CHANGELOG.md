# Changelog

## 0.6.1
- `Resync` (C2): a client with a read grant on a file gets the whole file as `commy-wire` range frames (at most 1 MiB each, offset order, all carrying the service's current `seq`), then `ResyncDone{seq, size}`. The connection loop sends queued binary frames before the text reply. Unknown file `NOT_FOUND`; another tenant's file or no read grant `PERMISSION_DENIED`. `ServiceRecord` gains `seq`; `ClientSession` gains `outbound_frames`.

## 0.6.0 (breaking)
- `GetServiceFilePath` is real (C1) and now uses the `commy-wire` 0.2.1 messages: it needs a `request_id` and returns `ServiceFilePath` with the service's `file_id`. `path` and `nonce_path` are present only when the service has been opened to local mapping (`Server::set_trusted_local`, default off) and the caller holds a write grant (`ServiceWrite`, `ServiceAdmin` or `TenantAdmin`).
- `ConfirmLocal` checks the per-connection nonce (random, in a 0600 side file in the tenant directory), deletes it on the first attempt, and answers `ConfirmLocalAck` or `PERMISSION_DENIED`. At most 16 unconfirmed nonces per session (`RATE_LIMITED`); unconfirmed nonce files are removed on `Disconnect` and swept after 60 s.
- New replies use the `commy-wire` error spelling (`NOT_FOUND`, `PERMISSION_DENIED`, `INVALID_REQUEST`, `RATE_LIMITED`); the rest of the server still uses the old spellings until the spelling sweep.
- `ServiceRecord` gains `file_id` and `trusted_local`; `ClientSession` gains `pending_nonces` and `local_files`; `Server` gains `next_file_id`.

## 0.5.0 (breaking)
- `CreateService` requires `size` (bytes, a positive integer). The file is rounded up to whole 4 KiB pages and may not exceed the server maximum (default 10 MiB; `COMMY_MAX_SERVICE_SIZE` until config-file support exists). A missing, zero, negative or non-integer size is refused as `InvalidRequest`. The `Service` reply carries the final `size`.
- `Tenant::register_service` takes a `size` argument. `Server::max_service_size`, `Server::with_max_service_size`, `service_file_size`, `ServiceSizeError`, `PAGE_SIZE`, `DEFAULT_MAX_SERVICE_SIZE` are new.

## 0.4.1
- Untrack committed example build output (fixes "filename too long" on Windows clones). This is the release that ships the 0.4.0 security fix.

## 0.4.0 (breaking, security)
- Every operation except `Heartbeat`/`Disconnect` needs an authenticated session; `Authenticate` is refused unless `COMMY_ALLOW_DEV_AUTH=1`; tenant ids are validated (`[A-Za-z0-9_-]{1,64}`) before use in paths; `DeleteTenant` removes the tenant; services are checked against the session's tenant. GHSA-5q28-5hrh-w432, GHSA-rwg3-grc8-336p.
