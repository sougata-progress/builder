# Extending `artifactory-client`

A practical guide for adding new behaviour to this crate without breaking
existing functionality.

---

## Module map

```
src/
├── lib.rs       – crate root; re-exports the public surface
├── config.rs    – ArtifactoryCfg struct + constants
├── client.rs    – ArtifactoryClient (upload / download / delete)
└── error.rs     – ArtifactoryError enum + From impls
```

---

## Adding a new API method

1. **Add the method to `client.rs`** inside `impl ArtifactoryClient`.

   Follow the existing pattern:
   ```rust
   /// One-line summary of what this does.
   ///
   /// Longer explanation if needed (edge cases, panics, idempotency).
   pub async fn my_operation(
       &self,
       ident: &PackageIdent,
       target: PackageTarget,
   ) -> ArtifactoryResult<()> {
       let url = self.url_path_for(ident, target);
       // ... call self.inner.get/put/post/delete ...
   }
   ```

2. **Add a new error variant** in `error.rs` only if the operation can fail in
   a way not already covered by `ArtifactoryError`. Add the corresponding
   `fmt::Display` arm and `From` impl.

3. **Re-export from `lib.rs`** if you add a new public type.

4. **Write at least one unit test** (see section below).

---

## Adding a new config field

1. Add the field to `ArtifactoryCfg` in `config.rs` with a `///` doc comment.
2. Add a matching `DEFAULT_*` constant if a sensible default exists.
3. Update `Default::default()` to populate the field.
4. Update `ArtifactoryClient::new` to consume the field (validate early — before creating
   headers or calling `HttpClient::new` — so the error is returned synchronously).
5. Add an assertion in `default_cfg_uses_expected_constants` to pin the default.

### Example: the `require_https` toggle

`require_https: bool` is a minimal feature toggle added to illustrate this pattern:

- **Default `false`** — preserves existing behaviour (HTTP is accepted for local dev).
- **`true`** — `new()` returns `Err(InvalidConfig(...))` before any network call if
  `api_url` doesn't start with `https://`.
- Set in TOML: `require_https = true` under `[artifactory]`.
- Three unit tests cover the toggle: OFF+HTTP=ok, ON+HTTP=err, ON+HTTPS=ok.

Apply the same pattern for any new optional validation or behaviour gate.

---

## Testing strategy

### Unit tests (no network, no disk)
Add a `#[cfg(test)] mod tests` block at the bottom of the relevant file.

- **`config.rs`** – test `Default` values and constant correctness.
- **`error.rs`** – test `Display` output for each variant using
  `format!("{}", err)`.
- **`client.rs` `url_path_for`** – test URL shape with a known ident/target.
  This function is private; test it from inside the `#[cfg(test)]` module in
  the same file.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_path_for_produces_expected_shape() {
        // build a minimal client without a real HTTP connection
        // ... (use ArtifactoryCfg::default() and skip new() if HttpClient
        //     construction is hard to mock; alternatively extract url_path_for
        //     into a free function for testability)
    }
}
```

### Integration / mock-HTTP tests
Use [`wiremock`](https://crates.io/crates/wiremock) (add as `dev-dependency`)
to spin up a local mock server and assert the correct HTTP verb, path, and
headers are sent:

```toml
# Cargo.toml
[dev-dependencies]
wiremock = "0.6"
tokio = { version = "*", features = ["macros", "rt-multi-thread"] }
```

```rust
#[tokio::test]
async fn upload_sends_put_to_correct_path() {
    use wiremock::{matchers::{method, path}, Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/artifactory/habitat-artifact-store/..."))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let cfg = ArtifactoryCfg { api_url: server.uri(), ..Default::default() };
    let client = ArtifactoryClient::new(cfg).unwrap();
    // ... call client.upload(...)
}
```

### What to verify per method

| Method | Must verify |
|---|---|
| `upload` | PUT verb; correct URL; 4xx → `ApiError`; file-not-found → `IO` error |
| `download` | GET verb; response body written to `destination_path`; 4xx → `ApiError` |
| `delete` | DELETE verb; 404/410 treated as success; other 4xx → `ApiError` |
| `url_path_for` | URL contains `repo`, all ident segments, target segments, and hart filename |

---

## Checklist before opening a PR

- [ ] New/changed public items have `///` doc comments.
- [ ] `cargo test -p artifactory-client --lib` passes.
- [ ] `cargo clippy -p artifactory-client -- -D warnings` produces no output.
- [ ] `cargo fmt -p artifactory-client -- --check` passes.
- [ ] No `unwrap()` in production paths (only in tests or with a `# Panics` doc).
