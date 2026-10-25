# Patent sweep: commy 0.27.3 @ d27e1f6
Date: 2026-10-25; Run by: commy lane, Claude Sonnet 5.5; Method: AI-assisted web search and read; NOT legal advice; NOT a freedom-to-operate opinion. Process: `PATENT-SWEEP-PROCESS.md` in the portfolio root.

**Coverage warning.** This record is a first pass. Independent claims were read for 3 patents (marked *read*); every other row was judged from abstracts, snippets or description text only and is marked *claims not read*. Those verdicts are provisional and the rows are listed under "Follow-up".

## Ideas swept
1. A shared-memory service is an unstructured memory-mapped file created zero-filled at a requested size (rounded to whole pages); the server writes no header, allocator or bookkeeping into it, so the meaning of the bytes belongs entirely to the programs sharing it.
2. A same-machine client is offered direct mapping of the service file only if (a) an administrator opened the service to local mapping and (b) the client holds a write grant, because the server cannot observe stores into a mapping.
3. Proof that a remote-looking client shares the server's filesystem: the server writes a per-connection random nonce to a side file in the tenant directory (never in the service file), the client reads it and echoes it over the already-authenticated network connection, and the server deletes it on first use; stale unconfirmed nonces are swept by TTL and each session is capped on outstanding nonces.
4. Clients that do not share the filesystem hold a byte-for-byte mirror: one full resync of the file, then only the changed byte ranges (with the bytes) per flush.
5. A flush from any member is forwarded to every other member of the file: locally mapped members receive range descriptors only (they already see the stores), mirror members receive ranges plus bytes read from the file at forwarding time, split into frames of bounded payload; ranges are sorted and overlapping/adjacent ranges merged into a minimal list first.
6. Write ordering is assigned by the server: one sequence number per flush, stamped on every range frame of that flush, giving all mirrors a single total order of changes to a file.
7. Changed ranges in a locally mapped file are detected by trapping page-faults (write detection via the dirty-ranges crate), with a scan-and-compare fallback.
8. Live replacement of a service's backing file ("rotation") with a drain barrier: on resize, every member flushes and acknowledges on the old file (stragglers dropped at a deadline) before the requester fills a new file, then commits or aborts; only the requester may answer, once, and only after the new file is prepared.
9. Revoking a client's access to a service forces a rotation of that service's file (the file moves to a new name/location) and drops the revoked client, so a client that retained a mapping or path of the old file loses all access to later data; revocation rotation bypasses the spacing and rate limits but remains one-at-a-time per service.
10. Rotation abuse limits: minimum spacing between rotations of one service and a cap on rotations started per requester per time window, with the spacing configurable at runtime.
11. Access control as per-client, per-tenant, per-service grants with actions read/write/resize/admin where admin implies all and write implies read; a tenant-wide grant is shorthand for every service, and removing one service from a client holding only a tenant-wide grant is stored as a deny on that service that is evaluated before any grant.
12. Authentication-supplied permissions are imported as tenant-wide grants so every service check goes through the single grant engine (additive, idempotent).
13. The authenticated identity is taken from the credential's subject, never from what the client claims; a development verifier that trusts the client is enabled only by an environment switch that the configuration file is forbidden to set.
14. Grant changes are kept as an operation log and periodically compacted into a snapshot (every N changes) that restores the full grant state, with durable storage and fail-closed recovery.
15. Hash-chained, append-only audit log of server events: verification detects modification, insertion and reordering of retained events; the chain head can be exported to detect truncation; write failures are counted and never block the audited operation.
16. Hot reload of server configuration by polling the file's modification time and size: the whole file is parsed and validated; an invalid file (syntax, unknown key, forbidden key) is rejected whole and the old settings kept; only a whitelist of live-safe keys is applied, a change to any other key is recorded as awaiting restart, every outcome is an audit event, and the environment still overrides the file.
17. A configuration file that may never hold secrets or insecure switches: such keys, and any unknown key, make start-up fail rather than be ignored.
18. Service files live under an unlisted per-tenant directory with long random file names as a location-secrecy barrier (stated limit: no per-file OS permissions).
19. (Cargo feature `clustering`, experimental, unused by default) Multi-server replication using vector clocks, conflict resolution, heartbeat failover and session persistence.

## Queries (source, query, date; all 2026-10-25)
Web search (general index, which returns Google Patents, USPTO and Justia pages) and Google Patents page fetches; no USPTO/EPO/WIPO search form was queried directly.
1. shared memory mapped file synchronization, remote mirror, changed byte ranges forwarded to other clients, sequence number (ideas 4-6)
2. proof of shared filesystem: nonce file written by server, read by client, echoed over authenticated connection (idea 3)
3. revoke access to shared memory region by relocating or replacing the backing file; drain barrier; resize (ideas 8-9)
4. hash-chained audit log, tamper-evident, modification / insertion / reordering (idea 15), twice (second restricted to patents.google.com)
5. dirty memory pages, page-fault write trapping, replicate changed ranges of a mapped file (idea 7)
6. access revocation of a shared memory segment, client mapping, replacement region (ideas 8-9)
7. role-based access control, deny takes precedence over grant, tenant-wide permission with resource-specific deny (ideas 11-12)

Not searched at all: ideas 1, 2, 10, 13, 14, 16, 17, 18, 19.

## Hits
| # | patent | title | assignee | status | claim(s) read | overlap | our difference | verdict |
|---|--------|-------|----------|--------|---------------|---------|----------------|---------|
| 1 | US 8,954,697 B2 (US20120036334) | Access to shared memory segments by multiple application processes | Red Hat | active, adj. expiry 2033 (Google's assumption) | *read*: independents 1, 5, 9 | "detecting ... a page fault in response to receiving a write request" (idea 7, in the SDK / `dirty-ranges`) | every claim also needs: creating a physical-memory copy of the original page, replacing the stored page data with the copy on a first synchronization request, and updating a *second* process's page table to the replaced data on its synchronization request. Commy never copies pages or replaces stored pages; it reports changed ranges and the server forwards them | clear |
| 2 | US 11,150,845 B2 | Methods and systems for servicing data requests in a multi-node system | EMC | active | *read*: independents 8, 15 (claim 1 not read; claims 4-7 depend on it) | remote-node data copied to a local node (ideas 4-5) | claims require a space manager resolving a virtual address to a logical volume managed by a remote node, a data layout request returning a *physical address*, a copy request, and a virtual-to-physical mapping. Commy has no logical volumes, layout requests or physical-address mapping | clear |
| 3 | US 11,392,464 B2 | Methods and systems for mirroring and failover of nodes | EMC | active, adj. expiry 2039 | *read*: independents 6, 11 (claim 1 not read; its dependents 2-5 have the same shape) | heartbeat and failover (idea 19, feature `clustering`, off by default) | claims require a failover node updating a "sparse virtual space" managed by a "file system container" to flip a file system from local to remote, and re-synchronising only data written during the outage. Commy has neither | clear |
| 4 | US 6,952,722 B1 | peer mapping system call to map changes in shared memory to all users | Cisco | expired 2023 | summary only | newly allocated shared memory mapped into other processes | expired | clear (expired) |
| 5 | US 5,987,506 | Remote access ... globally addressable storage environment | Mangosoft | expired 2016 | *claims not read* | shared memory across networked computers, byte-range locks | expired | clear (expired) |
| 6 | US 5,978,475 | Event auditing system | Counterpane | expired 2017 | *claims not read* | each log entry linked to its predecessor (idea 15) | expired | clear (expired) |
| 7 | US 10,878,079 B2 | Identity cloud service authorization model with dynamic roles and scopes | Oracle | active, expiry 2038 | *claims not read* | text says a resource-level policy and deny can outrank inherited grants (idea 11) | not read | **provisional**: follow-up |
| 8 | US 7,890,531 B2 | resolving permission for role activation operators | not recorded | status not checked | *claims not read* | grant-vs-deny precedence (idea 11); this one says a grant wins unless "override deny" | not read | **provisional**: follow-up |
| 9 | US 9,338,013 B2 / US 2016/0254906 | Verifiable redactable audit log | not recorded | status not checked | *claims not read* | hash chain over log entries, signed head (idea 15) | commy's chain is not redactable and not signed; claims not read | **provisional**: follow-up |
| 10 | US 2010/0115284 A1; EP 2897051 A2; WO 2020/104935 A1; US 2013/0262871 A1 | tamper detection for a log; verifiable audit log; tamper-proof record chain; tamper-proof camera logs | not recorded | status not checked (some are applications) | *claims not read* | prior-entry hash inside next entry (idea 15) | not read | **provisional**: follow-up |
| 11 | US 5,649,099 A | delegating access rights; revocable access control objects | not recorded | very old, likely expired (not checked) | *claims not read* | revocable rights (idea 9) | not shared memory | provisional |

Non-patent prior art found (not hits, but it limits what could be claimed): the D-Bus `nonce-tcp` transport, where the client reads a nonce from a file the server publishes and sends it over the socket (idea 3); QNX `shm_revoke()`, revoking other processes' mappings of a shared-memory object (idea 9); open-source hash-chained audit-log libraries that detect modification, insertion and reordering and note that truncation needs an external anchor (idea 15); Linux write-protect / dirty-bit write tracking and KVM live-migration dirty-page logging (idea 7).

## Needs review (to the PM)
None confirmed. No hit showed an overlap on every element of an independent claim that was actually read.

## Follow-up (this record is not complete until these are done)
1. Read the independent claims of hits 5-11. The USPTO PDFs (`image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/<number>`) are scanned images; render the last pages and read them.
2. Search ideas 4-6 (sequence-numbered range forwarding to mirrors) and 8-10 (rotation with a drain barrier, rate limits) properly: the queries returned storage-mirroring and NFS patents, not close matches, which is weak evidence of absence.
3. Search ideas 1, 2, 13, 14, 16, 17, 18 and 19.
4. Query USPTO and Espacenet directly (this pass used a general web search and Google Patents pages only); check legal status of hits 8-11.

## Limitations
AI-assisted search with a general web search tool, not a patent database query interface: recall is unknown and each query returned few patents. Claim text was read from scanned US PDFs for three patents only. Patent status is Google Patents' own "assumed" status. Other jurisdictions (EP, CN, JP, KR) were not searched. This is a due-diligence read, not legal advice and not a freedom-to-operate opinion; a lawyer's review is a separate purchase.
