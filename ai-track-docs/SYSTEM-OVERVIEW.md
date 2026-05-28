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

## Key Conventions
- **Rust edition 2021**; toolchain pinned in `rust-toolchain`.
- Protobuf message code is **generated** during build – never edit `components/builder-protocol/src/message/` by hand.
- Secrets and tokens must never be committed; use environment variables or Habitat config injection.
- All PR branches follow the pattern `<JIRA-ID>` (e.g., `BLDR-1234`).

## Related Docs
- [Architecture diagram](architecture.mmd)
- [Build & Test guide](build-test.md)
- [Developer environment](../dev-docs/dev-environment.md)
