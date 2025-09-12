PR #13 - FFI Warnings Cleanup — Test Run Summary

What I changed in this branch:

- Resolved merge conflicts and removed duplicate attributes in `src/ffi/working_sync.rs`.
- Restored the missing exported FFI function `commy_malloc` in `src/ffi/memory.rs`.
- Added unit tests covering null pointer, zero port, and invalid/stale-handle cases for FFI callers.
- Addressed reviewer comments: removed temporary vendor artifact created during merge.

Local test run (commands run locally):

- Set TEST_ENV=1 and ran `cargo test` on Windows PowerShell.
- Result: All tests passed locally. Key outputs:
  - FFI tests: created multiple mesh coordinators successfully and handled null node_id/zero port cases.
  - Unit tests: 100% pass for added FFI edge-case tests.
  - Note: Numerous unrelated compiler warnings remain (unused imports, dead code, and unused_unsafe). These are low-risk and planned for a follow-up PR.

Next steps:

- Merge or request additional reviewer changes for this PR.
- I created a separate branch `chore/lint-cleanup` with small lint fixes (remove unnecessary `unsafe` in examples/tests and fix a corrupted demo) and will open a separate PR for that.

If you'd like, I can also update the PR description to include CI logs or a short diff for the key files changed.
