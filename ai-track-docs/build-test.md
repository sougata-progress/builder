# Build & Test Guide

## Local Test Script (`test/test-artifactory-client.sh`)

A self-contained script that runs all four reliability checks for `artifactory-client`
with no external dependencies (no Habitat packages, no Docker, no network):

```bash
# From the repository root:
bash test/test-artifactory-client.sh

# Verbose mode (prints individual test names and stdout):
bash test/test-artifactory-client.sh --verbose
```

### What it runs, in order

| Step | Command | Fails fast? |
|---|---|---|
| 1. Format check | `cargo fmt -p artifactory-client -- --check` | Yes |
| 2. Build | `cargo build -p artifactory-client` | Yes |
| 3. Unit tests | `cargo test -p artifactory-client --lib` | Yes |
| 4. Lint | `cargo clippy -p artifactory-client -- -D warnings` | Yes |

Each step prints `[PASS]` or `[FAIL]`. The script exits `0` only when all steps pass;
any failure exits `1` and lists which steps failed. This makes it safe to use as a
pre-push hook or in a future CI job.

### Why CI doesn't currently run Rust tests

The main CI workflow ([`.github/workflows/ci-main-pull-request-checks.yml`](.github/workflows/ci-main-pull-request-checks.yml))
delegates to a central shared Action at `chef/common-github-actions` with
`unit-tests: false` and `build: false`. The existing `test/run_cargo_test.sh`
requires Habitat packages and a Buildkite environment. This local script fills
that gap without modifying the central CI configuration.

### Using as a git pre-push hook

```bash
# Install once per local clone:
cp test/test-artifactory-client.sh .git/hooks/pre-push
chmod +x .git/hooks/pre-push
```

---

## Quick Reference – Individual Commands

```bash
# 1. Build the crate only (no network services needed)
cargo build -p artifactory-client

# 2. Run its tests (all unit tests, no network or DB required)
cargo test -p artifactory-client

# 3. Run a single named test by filter
cargo test -p artifactory-client default_cfg_uses_expected_constants

# 4. Clippy for the crate
cargo clippy -p artifactory-client -- -D warnings

# 5. Format check
cargo fmt -p artifactory-client -- --check

# 6. Build + test the whole workspace (slower, requires no live services for unit tests)
cargo build --workspace
cargo test --workspace
```

> **Toolchain**: pinned to `1.91.1` in `rust-toolchain`. Run `rustup show` to confirm the active toolchain matches before building.

---

## Lint Policy (`artifactory-client`)

### How lints are layered

There are two independently-controlled lint layers:

| Layer | Mechanism | Scope |
|---|---|---|
| **Crate-level attributes** | `#![warn(...)]` / `#![allow(...)]` in `src/lib.rs` | Our crate's source only — never affects dependencies |
| **CLI flag** | `-D warnings` passed to `cargo clippy` | Turns every remaining warning into a build error |

The CLI flag `-W clippy::pedantic` would apply pedantic lints to every crate in the
compilation graph, including dependencies that are not under our control.
**Do not use it on the CLI.** Use the crate-level attribute for targeted pedantic lints.

### Active lint attributes in `src/lib.rs`

```rust
#![warn(clippy::pedantic)]                  // broad style/correctness coverage
#![allow(clippy::module_name_repetitions)]  // ArtifactoryClient, ArtifactoryCfg etc.
                                             // intentionally repeat the module name
#![allow(clippy::missing_errors_doc)]       // error conditions documented in prose;
                                             // a formal # Errors section adds nothing
```

### Running lints

```bash
# Standard lint check used by the local test script:
cargo clippy -p artifactory-client -- -D warnings

# View pedantic findings across the crate (safe — pedantic only applies to our files):
cargo clippy -p artifactory-client -- -D warnings 2>&1 | grep "artifactory-client"
```

### Suppressing a lint locally

If a single site needs to override a crate-level warn, use a line-level `#[allow(...)]`
with an explanatory comment, never a blanket crate-level `#![allow(...)]`:

```rust
#[allow(clippy::cast_precision_loss)] // iteration count fits comfortably in f64
let pct = (done as f64 / total as f64) * 100.0;
```

---

## Prerequisites

| Tool | Purpose |
|---|---|
| Rust toolchain (see `rust-toolchain`) | Compile all Rust components |
| `protoc` ≥ 3.x | Generate Protobuf bindings |
| Docker & Docker Compose | Local backing services (Postgres, Minio, Memcached) |
| Node.js 18+ & npm | Build and test `builder-web` |
| `cargo-nextest` (optional) | Faster parallel test runner |

## Building

### All Rust components
```bash
cargo build --workspace
```

### Single component
```bash
cargo build -p builder-api
```

### Frontend
```bash
cd components/builder-web
npm ci
npm run build
```

## Running Tests

### Rust unit tests
```bash
cargo test --workspace
```
or with nextest:
```bash
cargo nextest run --workspace
```

### Builder-db integration tests (requires running Postgres)
```bash
cargo test -p builder-db
```

### Frontend unit tests
```bash
cd components/builder-web
npm test -- --single-run
```

### Linting
```bash
# Clippy (Rust)
bash test/run_clippy.sh

# Formatting check
cargo fmt --all -- --check

# Shell scripts
bash test/shellcheck.sh
```

## Coverage Requirements
- Minimum **80 %** line coverage for Rust crates under `components/`.
- Use `cargo llvm-cov` or `cargo tarpaulin` to measure:
  ```bash
  cargo tarpaulin --workspace --out Html
  ```
- Frontend coverage is reported by Karma; target ≥ 80 % statements.

## Local Services via Docker Compose
```bash
# Start Postgres + Minio
docker compose -f components/builder-minio/docker-compose.yml up -d

# Apply database migrations
cargo run -p builder-db --bin builder-migrate
```

## CI / CD Notes
- CI is triggered on every PR push.
- Fast-pass script: `support/ci/fast_pass.sh` skips unaffected components.
- Build artifacts are stored in S3-compatible storage configured via environment variables.

## Common Issues

| Symptom | Likely Cause | Fix |
|---|---|---|
| `can't find crate for 'message'` | Protobuf not generated | Run `cargo build -p builder-protocol` first |
| `connection refused` in db tests | Postgres not running | `docker compose up -d` |
| `npm ERR! peer dep` | Node version mismatch | Use Node 18 LTS |

## Viewing Structured Logs (`artifactory-client`)

All three public operations (`upload`, `download`, `delete`) emit log lines using
a consistent key=value field schema:

| Field | Type | Meaning |
|---|---|---|
| `op` | string | Operation name: `upload`, `download`, `delete` |
| `status` | string | `ok`, `error`, or `not_found` |
| `elapsed_ms` | integer | Wall-clock time of the HTTP round-trip in milliseconds |
| `http_status` | integer | Raw HTTP status code returned by Artifactory |
| `url` | string | Full request URL (present on `debug!` lines and errors) |
| `err` | string | Rust error message (present only on transport-level errors) |

### Log levels used

| Level | When |
|---|---|
| `debug` | Request URL before send — verbose, off by default |
| `info` | Successful completion and idempotent 404/410 deletes |
| `error` | HTTP errors and transport failures |

### Enable and view logs locally

```bash
# Show info-level logs for artifactory-client only
RUST_LOG=artifactory_client=info cargo run -p builder-api

# Show all debug-level logs for the crate (verbose)
RUST_LOG=artifactory_client=debug cargo run -p builder-api

# Show info for everything, debug for this crate
RUST_LOG=info,artifactory_client=debug cargo run -p builder-api
```

> `RUST_LOG` uses the **module path** as the filter key, which is the crate name
> with hyphens replaced by underscores: `artifactory-client` → `artifactory_client`.

### Example log output

```
# Successful upload
INFO artifactory_client::client  op=upload status=ok elapsed_ms=142 http_status=200

# Failed download (server error)
ERROR artifactory_client::client  op=download status=error elapsed_ms=31 http_status=500

# Idempotent delete (artifact already gone)
INFO artifactory_client::client  op=delete status=not_found elapsed_ms=12 http_status=404 note=already_removed

# Transport-level failure
ERROR artifactory_client::client  op=upload status=error elapsed_ms=5002 url=http://… err=connection timed out
```

### Filtering in production (JSON logging pipeline)

If `builder-api` is deployed with a JSON log formatter (e.g. `tracing-subscriber`
in JSON mode), the key=value fields parse directly into structured JSON. To extract
slow operations:

```bash
# Find any Artifactory op that took > 2000 ms (using jq)
journalctl -u builder-api | jq 'select(.fields.elapsed_ms > 2000)'
```
