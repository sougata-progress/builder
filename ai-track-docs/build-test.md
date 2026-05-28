# Build & Test Guide

## Quick Reference – Exact Commands for `artifactory-client`

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
