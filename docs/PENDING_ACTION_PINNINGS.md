# Pending Action Pinning

This file lists GitHub Actions usages across the repository that currently reference floating major versions (e.g. `@v1`, `@v4`, `@v3`) and should be pinned to immutable SHAs or specific tags for reproducible CI and to satisfy security scanners (CodeQL warnings).

Strategy:

- Prefer an immutable commit SHA for ecosystem actions when possible, e.g. `actions/checkout@ff7abcd0c3c05ccf6adc123a8cd1fd4fb30fb493`.
- For official actions that publish signed releases (actions/checkout, actions/cache, setup-node, setup-python), use the latest stable minor tag or pin to a recent SHA.
- If pinning in a workflow is risky, open a follow-up PR that pins one workflow at a time with CI verification.

Workflows and docs with floating action versions discovered via a repo scan:

- `.github/workflows/ci.yml`:
  - `actions/checkout@v4` (consider pinning to a specific SHA)
  - `actions/cache@v4`
  - `actions/upload-artifact@v4`
  - `actions/setup-python@v5`
  - `actions/setup-node@v4`

- `.github/workflows/ffi-smoke-test.yml`:
  - `actions/checkout@08eba0b27e820071cde6df949e0beb9ba4906955` (already pinned to SHA)
  - `actions/cache@0400d5f644dc74513175e3cd8d07132dd4860809` (already pinned to SHA)

- `.github/workflows/capnp_codegen.yml`:
  - `actions/checkout@ff7abcd0c3c05ccf6adc123a8cd1fd4fb30fb493` (pinned)
  - `actions/upload-artifact@v4`

- `docs/llm_code_review.md` (examples show `actions-rs/toolchain@v1`, `actions/checkout@v4`, `actions/cache@v4`, `actions/setup-python@v5`, `actions/setup-node@v4`) — update examples or annotate TODO to pin in real workflows.

- `archive/status_sources/TESTING_ROADMAP.md` (example uses `actions/checkout@v3`) — update examples.

Next steps (planned):

1. Create small, focused follow-up PRs that pin each workflow one at a time and run CI. Document the pin used and the reason in the commit message.
2. Update documentation examples to show pinned SHAs or add a clarifying note that examples are illustrative and shouldn't be copied verbatim into production workflows without pinning.
3. Optionally, introduce a CI job that scans workflows for floating action versions and fails if found.

If you'd like, I can open the first follow-up PR now that merges the docs file and updates the example docs to use pinned SHAs/placeholders.
