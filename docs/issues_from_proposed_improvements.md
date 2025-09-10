Remaining items from `PROPOSED_IMPROVEMENTS.md`

This is an auto-generated list of work items to create GitHub issues for.

1. Property-based tests
   - Add proptest suites for core manager invariants (file ID allocation, reuse, concurrent creates).
2. Simple object pool
   - Provide a small, low-overhead object pool for frequently allocated structures (e.g., Buffer, Transport objects).
3. Auth provider integration
   - Ensure `SharedFileManager` strictly uses `AuthProvider` abstraction and remove any non-deterministic fallback behavior.
   - Add end-to-end integration tests that exercise `RealAuthProvider` against the deployed `auth-framework`.
4. Stubs and mocks inventory
   - Enumerate and implement test doubles for network, storage, and FFI layers.
5. CI/test improvements
   - Add targeted CI jobs for manager feature tests and proptest.
6. Documentation
   - Expand `docs/auth-refactor.md` and add migration guide for plugin and FFI changes.

If you'd like, I can create one GitHub issue per item and populate them with the corresponding files, tests, and suggested labels.
