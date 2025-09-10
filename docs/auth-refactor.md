Auth provider refactor

This commit introduces a testable `AuthProvider` abstraction which wraps the
existing `auth-framework` runtime. It provides:

- `AuthProvider` async trait for pluggable auth validation.
- `RealAuthProvider` that wraps `auth_framework::AuthFramework`.
- `MockAuthProvider` for deterministic tests.

The manager now accepts an `Arc<dyn AuthProvider>` so unit tests can inject a
mock provider, and production code uses `RealAuthProvider` that delegates to
`auth-framework`.

This document accompanies the focused commit on branch `fix/auth-validate`.
