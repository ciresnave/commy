# Clustering: experimental and not in use

**Commy has no supported multi-server mode.** The source tree contains `src/server/clustering`
(about 6,400 lines: peer registry, heartbeats, replication, failover, vector clocks, snapshots). It
was written for the original variable-based design and has not been reworked for the bare-bytes
design (range frames, rotation, grants). The server's request path does not call it:

- `COMMY_CLUSTER_ENABLED=true` makes the binary build a `ClusteringConfig` and print it. Nothing
  connects to the nodes in `COMMY_CLUSTER_NODES`, nothing is replicated, and a second server shares no
  state with the first.
- Services, grants, sessions and rotations live in the process (and its state directory) only.

Do not deploy several Commy servers expecting high availability or shared services: clients of
different servers would see different, unrelated services.

## What to do instead today

Run one server per site and give it a durable state directory (`docs/DEPLOYMENT.md`). A restarted
server restores its services and grants; clients resynchronise.

## Status

The scope of a future design is not decided and is held until the FAM to Synapse merge settles what
replication belongs where. The code is kept for reference; `ROADMAP.md` row R10 tracks it, and a
later release may put it behind a Cargo feature that is off by default.
