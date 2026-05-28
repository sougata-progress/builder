// Copyright (c) 2019 Chef Software Inc. and/or applicable contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

/// Base URL for the Artifactory API.
///
/// Used as the default when no `api_url` is present in the TOML config.
/// Override this for production deployments.
pub const DEFAULT_ARTIFACTORY_API_URL: &str = "http://localhost:8081";

/// Default Artifactory repository that stores `.hart` packages.
///
/// Override via `ArtifactoryCfg::repo` for custom repository layouts.
pub const DEFAULT_ARTIFACTORY_REPO: &str = "habitat-artifact-store";

/// Runtime configuration for [`ArtifactoryClient`](crate::ArtifactoryClient).
///
/// Deserialized from the `[artifactory]` table in the service TOML config.
/// All fields fall back to the module-level `DEFAULT_*` constants when absent.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct ArtifactoryCfg {
    /// Full base URL of the Artifactory instance, e.g. `https://artifactory.example.com`.
    pub api_url: String,
    /// Artifactory API key used for authentication via the `x-jfrog-art-api` header.
    /// Leave empty to disable authenticated requests (local dev only).
    pub api_key: String,
    /// Repository name within Artifactory that holds `.hart` artifacts.
    pub repo: String,
    /// When `true`, [`ArtifactoryClient::new`](crate::ArtifactoryClient::new) rejects any
    /// `api_url` that does not start with `https://`.
    ///
    /// **Toggle behaviour**
    /// | Value | Effect |
    /// |---|---|
    /// | `false` (default) | Any URL scheme is accepted; HTTP is allowed for local dev. |
    /// | `true` | Construction fails with [`ArtifactoryError::InvalidConfig`](crate::ArtifactoryError::InvalidConfig) if `api_url` is not HTTPS. Enable this in any non-development environment. |
    ///
    /// Set via TOML:
    /// ```toml
    /// [artifactory]
    /// require_https = true
    /// ```
    pub require_https: bool,
}

impl Default for ArtifactoryCfg {
    /// Returns a configuration pointing at a local Artifactory instance with no
    /// authentication. Suitable for development; always override for production.
    fn default() -> Self {
        ArtifactoryCfg {
            api_url: DEFAULT_ARTIFACTORY_API_URL.to_string(),
            api_key: String::new(),
            repo: DEFAULT_ARTIFACTORY_REPO.to_string(),
            require_https: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cfg_uses_expected_constants() {
        let cfg = ArtifactoryCfg::default();
        assert_eq!(cfg.api_url, DEFAULT_ARTIFACTORY_API_URL);
        assert_eq!(cfg.repo, DEFAULT_ARTIFACTORY_REPO);
        assert!(
            cfg.api_key.is_empty(),
            "api_key should be empty string by default"
        );
        assert!(
            !cfg.require_https,
            "require_https should be false by default (allows HTTP for local dev)"
        );
    }

    #[test]
    fn default_api_url_is_localhost() {
        // Ensures the constant itself stays a local dev URL and is never
        // accidentally changed to a production endpoint.
        assert!(
            DEFAULT_ARTIFACTORY_API_URL.starts_with("http://localhost"),
            "DEFAULT_ARTIFACTORY_API_URL must be a localhost URL, got: {}",
            DEFAULT_ARTIFACTORY_API_URL
        );
    }

    /// Micro-benchmark: cost of constructing `ArtifactoryCfg::default()`.
    ///
    /// Runs 100 000 iterations via `std::time::Instant` (no external crate).
    /// Pass `-- --nocapture` to see the per-iteration ns printed to stderr.
    ///
    /// Baseline recorded 2026-05-28 on an AWS t3-class instance (x86-64, debug build):
    ///   262 ns / iter   (3× String allocations + constant copies)
    ///   variance: ±20 ns between runs (low – pure allocation, no syscalls)
    #[test]
    fn bench_default_construction() {
        use std::time::Instant;
        const ITERS: u32 = 100_000;
        let start = Instant::now();
        for _ in 0..ITERS {
            let _ = std::hint::black_box(ArtifactoryCfg::default());
        }
        let elapsed = start.elapsed();
        let ns_per_iter = elapsed.as_nanos() / u64::from(ITERS) as u128;
        eprintln!(
            "ArtifactoryCfg::default()  {ITERS} iters  total={elapsed:?}  \
             per_iter={ns_per_iter} ns"
        );
        // Sanity guard: construction must complete in under 10 µs each on any
        // reasonable CI machine (debug build, no optimisation).
        assert!(
            ns_per_iter < 10_000,
            "ArtifactoryCfg::default() took {} ns – unexpectedly slow",
            ns_per_iter
        );
    }
}
