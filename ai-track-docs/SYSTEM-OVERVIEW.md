# Habitat Builder – System Overview

## Purpose
Habitat Builder is a public SaaS infrastructure for users of the [Habitat](https://habitat.sh) automation framework.
It provides:
- A web UI (`builder-web`) for browsing packages, origins, and build jobs.
- REST APIs (`builder-api`) consumed by the `hab` CLI client.
- Background build and artifact-management services.

## High-Level Component Map

| Component | Language | Role |
|---|---|---|
| `builder-api` | Rust | Primary REST API; auth, package, channel, job endpoints |
| `builder-api-proxy` | Config/Nginx | TLS termination and routing in front of the API |
| `builder-core` | Rust | Shared domain types, error handling, utilities |
| `builder-db` | Rust | Database access layer (PostgreSQL via Diesel) |
| `builder-datastore` | SQL/Shell | Schema migrations and seed data |
| `builder-protocol` | Rust + Protobuf | Wire-format message types (generated via `build.rs`) |
| `builder-web` | TypeScript/Angular | Browser UI |
| `builder-minio` | Config | S3-compatible artifact object storage |
| `builder-memcached` | Config | Session / fragment caching |
| `github-api-client` | Rust | GitHub API integration (webhooks, user info) |
| `oauth-client` | Rust | Generic OAuth 2.0 client |
| `artifactory-client` | Rust | Artifactory repository integration |

## Request Flow (simplified)

```
hab CLI / Browser
      │
      ▼
builder-api-proxy  (Nginx, TLS)
      │
      ▼
builder-api        (Rust Actix-web)
      ├─► builder-db  ──► PostgreSQL
      ├─► builder-minio ► S3 / MinIO
      └─► builder-memcached ► Memcached
```

## Language Breakdown (verified counts, 2026-05-28)

| Language | File count | Primary location |
|---|---|---|
| Rust | 111 `.rs` files | `components/*/src/` |
| TypeScript / Angular | 168 `.ts` files | `components/builder-web/app/` |
| SQL | 48 `.sql` files | `components/builder-db/src/`, `components/builder-datastore/` |
| Protobuf | 1 `.proto` file | `components/builder-protocol/protocols/` |
| Shell | ~20 scripts | `support/`, `test/` |
| HCL (Terraform) | 10 files | `terraform/` |

## Entry Points

| Entry Point | Path | Role |
|---|---|---|
| Rust binary | `components/builder-api/src/main.rs` | Only runtime executable; boots Actix-web HTTP server |
| Angular bootstrap | `components/builder-web/app/main.ts` | Browser UI entry point |
| Build-time generator | `components/builder-protocol/build.rs` | Compiles `.proto` → Rust – **do not edit** |
| Workspace build helper | `components/build-builder.rs` | Top-level `build.sh` script shim |

> **Assumption**: `builder-api/src/main.rs` is the sole long-running process.
> Verify with: `grep -r "fn main" components --include="*.rs" -l`

## Test Approach

### Rust
- **Inline unit tests**: `#[cfg(test)]` blocks inside source files. Present in:
  - `components/builder-api/src/config.rs`
  - `components/builder-api/src/server/services/memcache.rs`
  - `components/builder-api/src/server/services/s3.rs`
  - `components/builder-core/src/access_token.rs`
  - `components/builder-core/src/build_config.rs`
  - `components/builder-core/src/keys.rs`
  - `components/builder-db/src/models/origin.rs`
  - `components/github-api-client/src/client.rs`
  - `components/github-api-client/src/types.rs`
- **Integration tests**: `components/builder-db/tests/db/` (require a live Postgres connection).
- **CI scripts**: `test/run_cargo_test.sh`, `test/run_clippy.sh`, `test/shellcheck.sh`.

### Frontend
- 26 `.spec.ts` files run by **Karma** (`karma.conf.js`).
- `components/builder-web/app/actions.test.ts` follows a Jest-style convention.

### End-to-end
- `test/builder-api/` and `test/end-to-end/` – API-level tests driven by shell scripts.

### Coverage requirement
Minimum **80 %** line coverage. Measure with:
```bash
cargo tarpaulin --workspace --out Html          # Rust
cd components/builder-web && npm test -- --single-run  # Frontend (Karma)
```

> **Assumption**: No additional test frameworks beyond Cargo test + Karma are in use.
> Verify with: `find . -name "jest.config*" -o -name "vitest.config*" | grep -v node_modules`

## Low-Risk Modules for Modification

The following three modules have zero or minimal blast radius (no critical dependents, small surface, no running-service coupling):

| # | Module | Path | Lines | Internal dependents | Existing tests |
|---|---|---|---|---|---|
| 1 | `artifactory-client` | `components/artifactory-client/src/` | 317 | 0 | None |
| 2 | `oauth-client` | `components/oauth-client/src/` | 1,079 | 1 (`builder-api`) | None |
| 3 | `github-api-client` types | `components/github-api-client/src/types.rs` | 370 | `github-api-client` only | 1 block |

### Recommended: `artifactory-client`

**Path**: `components/artifactory-client/src/`

**Files**:
```
client.rs   – HTTP request/response logic (190 lines)
config.rs   – struct holding base URL + credentials (38 lines)
error.rs    – custom error enum (60 lines)
lib.rs      – re-exports (29 lines)
```

**Why it is the safest choice**:

1. **Zero internal dependents** – no other crate in the workspace imports it. A compile error here cannot cascade into `builder-api` or any shared library. Verify: `grep -r "artifactory" components --include="Cargo.toml"`
2. **Smallest crate in the repo** – 317 lines total. The entire module fits in a single reading session.
3. **No existing tests to regress** – starting from zero means you define the baseline. There is nothing already passing that you can accidentally break.
4. **Pure HTTP client pattern** – the logic is: build request → `reqwest` send → deserialize with `serde`. No shared mutable state, no database, no auth side effects. Straightforward to mock with `wiremock` or `mockito`.
5. **External boundary only** – it talks to Artifactory (an external service), not to internal builder services or the database. Failures are isolated to that integration path.

**Suggested first exercise**: Add `#[cfg(test)]` unit tests to `config.rs` (test struct defaults and validation) and `error.rs` (test `Display` impls), then add mock-HTTP tests to `client.rs`. This alone can hit 80 %+ coverage for the crate without touching any other component.

> **Assumption**: `artifactory-client` is not imported outside `components/`.
> Verify with: `grep -r "artifactory.client\|artifactory_client" components --include="*.rs" | grep -v "artifactory-client/"`

## Key Conventions
- **Rust edition 2018** (per crate manifests); workspace toolchain pinned in `rust-toolchain`.
- Protobuf message code is **generated** during build – never edit `components/builder-protocol/src/message/` by hand.
- Secrets and tokens must never be committed; use environment variables or Habitat config injection.
- All PR branches follow the pattern `<JIRA-ID>` (e.g., `BLDR-1234`).

## Related Docs
- [Architecture diagram](architecture.mmd)
- [Build & Test guide](build-test.md)
- [Developer environment](../dev-docs/dev-environment.md)
