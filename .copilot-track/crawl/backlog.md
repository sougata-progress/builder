# Backlog – `artifactory-client` & surrounding hardening
<!-- Generated 2026-05-28 from crawl-phase findings. Commit to track as living backlog. -->

Items are ordered by risk/impact. Each item is self-contained and sized to fit a
single PR on the `learn/crawl/*` branch series.

---

## BLDR-BL-01 · Enable Rust unit tests in the GitHub Actions CI workflow

**Priority**: High  
**Size**: S (config change + smoke-test job)  
**Component**: `.github/workflows/`

### Context
The main PR workflow ([`ci-main-pull-request-checks.yml`](.github/workflows/ci-main-pull-request-checks.yml#L58-L61))
explicitly disables both `build: false` and `unit-tests: false`.
The existing `test/run_cargo_test.sh` requires Habitat packages and a
Buildkite environment unavailable in GitHub Actions.
The new `test/test-artifactory-client.sh` script is self-contained but is
not wired into any CI trigger.

### Why it matters
Any failing unit test (including the new negative test added in ex5) is
invisible in CI. A regression can be merged without notice.

### Acceptance criteria
- [ ] A new GitHub Actions workflow (e.g. `.github/workflows/rust-unit-tests.yml`) runs
  `bash test/test-artifactory-client.sh` on every PR touching `components/artifactory-client/**`.
- [ ] The job uses `ubuntu-latest` with no Habitat package dependencies.
- [ ] The workflow uses `paths:` filtering so it does not slow unrelated PRs.
- [ ] The job completes in under 5 minutes on a cold runner (first-build caches enabled).
- [ ] A PR that introduces a failing test is blocked from merging.

### Code links
- [`.github/workflows/ci-main-pull-request-checks.yml`](.github/workflows/ci-main-pull-request-checks.yml#L58-L61)
  — `unit-tests: false` line to document or override
- [`test/test-artifactory-client.sh`](test/test-artifactory-client.sh) — script to invoke
- [`test/run_cargo_test.sh`](test/run_cargo_test.sh) — existing script (requires hab, not usable as-is)

---

## BLDR-BL-02 · Redact `api_key` from `ArtifactoryCfg`'s `Debug` output

**Priority**: High  
**Size**: XS (impl change + one test)  
**Component**: `components/artifactory-client/src/config.rs`

### Context
`ArtifactoryCfg` derives `#[derive(Debug)]`
([`config.rs`](components/artifactory-client/src/config.rs#L30)).
Any call to `format!("{:?}", cfg)` — in a panic message, a test assertion
failure, or a log line — will print the raw `api_key` value in plaintext.
This is a credential-leak risk in any environment where API keys are real.

### Why it matters
Structured logging was added in ex9. If a future developer adds
`debug!("{:?}", self)` inside `ArtifactoryClient`, the key leaks immediately.

### Acceptance criteria
- [ ] `#[derive(Debug)]` is removed from `ArtifactoryCfg`.
- [ ] A manual `impl fmt::Debug for ArtifactoryCfg` is added that renders
  `api_key` as `"***"` (or `"<redacted>"`) regardless of value.
- [ ] `api_url` and `repo` are printed normally.
- [ ] A unit test asserts `format!("{:?}", cfg)` does **not** contain the key value
  and **does** contain `"***"`.
- [ ] `cargo clippy` and `cargo test --lib` both pass.

### Code links
- [`config.rs:30`](components/artifactory-client/src/config.rs#L30) — `#[derive(Debug)]`
- [`config.rs:37`](components/artifactory-client/src/config.rs#L37) — `api_key` field
- [`EXTENDING.md`](components/artifactory-client/EXTENDING.md) — "No `unwrap()` in production paths" rule to extend with Debug note

---

## BLDR-BL-03 · Replace `expect` panic in `url_path_for` with a graceful error

**Priority**: Medium  
**Size**: S (signature change cascades to callers)  
**Component**: `components/artifactory-client/src/client.rs`

### Context
`url_path_for` calls `ident.archive_name_with_target(target).expect("ident is fully qualified")`
([`client.rs`](components/artifactory-client/src/client.rs#L254-L255)).
If an unqualified `PackageIdent` (missing version or release) is passed by
a caller, the process panics rather than returning a structured error.
The three public methods (`upload`, `download`, `delete`) all call this
function and have no way to recover.

### Why it matters
A panic in an async Actix-web handler crashes the worker thread and may
cause a 500 response storm. The existing `ArtifactoryError` enum already
has an `InvalidConfig` variant that could be reused or extended.

### Acceptance criteria
- [ ] `url_path_for` is changed from `-> String` to `-> ArtifactoryResult<String>`.
- [ ] `archive_name_with_target` failure is mapped to `ArtifactoryError::InvalidConfig`
  with a message identifying the un-qualified ident.
- [ ] All three callers (`upload`, `download`, `delete`) propagate the error with `?`.
- [ ] A unit test passes an intentionally un-qualified `PackageIdent` to one
  of the public methods (via a helper or by making `url_path_for` testable) and
  asserts the result is `Err(InvalidConfig(_))` rather than a panic.
- [ ] `#[should_panic]` is never used — only `assert!(result.is_err())`.
- [ ] `cargo test --lib` and `cargo clippy -- -D warnings` both pass.

### Code links
- [`client.rs:252-261`](components/artifactory-client/src/client.rs#L252) — `url_path_for` definition
- [`error.rs:27`](components/artifactory-client/src/error.rs#L27) — `InvalidConfig` variant to reuse
- [`EXTENDING.md`](components/artifactory-client/EXTENDING.md) — "No `unwrap()` in production paths" rule

---

## BLDR-BL-04 · Add unit tests for `oauth-client` — at least one per OAuth provider

**Priority**: Medium  
**Size**: M (1,079 lines, 8 provider files, all untested)  
**Component**: `components/oauth-client/src/`

### Context
`oauth-client` has 1,079 lines across 8 provider files
(`a2.rs`, `active_directory.rs`, `azure_ad.rs`, `bitbucket.rs`,
`github.rs`, `gitlab.rs`, `okta.rs`, `client.rs`)
with **zero `#[cfg(test)]` blocks**. It is imported by `builder-api` —
the only runtime binary — making it the highest-risk untested module.

### Why it matters
OAuth token exchange and user-info parsing failures would silently pass
through as opaque errors to the end user. Each provider has its own
response shape; a shape change breaks authentication for all users of
that provider.

### Acceptance criteria
- [ ] Each provider file (`a2.rs` → `okta.rs`) has at least one `#[cfg(test)]`
  module with a test that parses a representative user-info JSON response
  (using a hardcoded string, no network).
- [ ] At least one negative test per provider verifies that a missing required
  field returns an appropriate error (not a panic).
- [ ] `oauth-client` compiles and `cargo test -p oauth-client --lib` passes.
- [ ] Line coverage for `components/oauth-client/src/` reaches ≥ 60 % (measured
  with `cargo tarpaulin -p oauth-client`). 80 % is the long-term target but
  this bootstraps the baseline from zero.
- [ ] No mocking of HTTP; test only the deserialization / mapping layer.

### Code links
- [`oauth-client/src/`](components/oauth-client/src/) — all provider files
- [`oauth-client/src/client.rs`](components/oauth-client/src/client.rs) — dispatch logic
- [`oauth-client/src/types.rs`](components/oauth-client/src/types.rs) — shared response types
- `builder-api/Cargo.toml` — confirms `oauth-client` is a direct dependency of the only binary

---

## BLDR-BL-05 · Add mock-HTTP tests for `ArtifactoryClient` upload / download / delete

**Priority**: Medium  
**Size**: M (new `dev-dependency` + 3 test functions)  
**Component**: `components/artifactory-client/src/client.rs`

### Context
All three public async methods — `upload`, `download`, `delete` — are
currently untested at the HTTP layer. The structured logs added in ex9
emit `op`, `status`, and `elapsed_ms`, but there is no test that
exercises the full request/response path, verifies the correct HTTP verb
is used, or checks that non-2xx responses produce the right error variant.

The `EXTENDING.md` guide already outlines a `wiremock`-based approach
([`EXTENDING.md`](components/artifactory-client/EXTENDING.md#L52-L75)).

### Acceptance criteria
- [ ] `wiremock` is added as a `[dev-dependencies]` entry in
  [`Cargo.toml`](components/artifactory-client/Cargo.toml).
- [ ] Three `#[tokio::test]` functions are added in a new `tests/integration.rs`
  file (or within `#[cfg(test)]` in `client.rs`):
  - `upload_sends_put_to_correct_url` — mock server expects `PUT`, returns `200`;
    asserts `Ok`.
  - `download_streams_body_to_file` — mock server returns a known byte sequence;
    asserts the destination file contains those bytes.
  - `delete_treats_404_as_ok` — mock server returns `404`; asserts `Ok(())`.
- [ ] At least one error path is covered: e.g. `upload_returns_api_error_on_500`.
- [ ] Tests do not require a real Artifactory instance; `wiremock::MockServer`
  provides the local HTTP endpoint.
- [ ] `cargo test -p artifactory-client` (all targets) passes.
- [ ] Line coverage for `client.rs` reaches ≥ 70 % after this item.

### Code links
- [`client.rs:75-270`](components/artifactory-client/src/client.rs#L75) — `upload`, `download`, `delete` implementations
- [`Cargo.toml`](components/artifactory-client/Cargo.toml) — where to add `wiremock` dev-dep
- [`EXTENDING.md`](components/artifactory-client/EXTENDING.md#L52) — `wiremock` example already written

---

## Backlog health notes

- Items 1–2 are security/reliability fixes; they should be prioritised above
  feature work.
- Items 3–5 are hardening and test coverage; they can proceed in any order.
- All items are scoped to zero-dependent or low-dependent modules — no changes
  to `builder-api` entry points are required for any of these items.
- Rotate the OAuth client secret referenced in `.secrets/habitat-env.sample`
  git history if credentials were ever real (see `SYSTEM-OVERVIEW.md` §Secret Hygiene).
