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

use std::{collections::HashMap, path::Path, time::Instant};

use crate::{
    config::ArtifactoryCfg,
    error::{ArtifactoryError, ArtifactoryResult},
};
use futures::stream::StreamExt;
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Body, Response,
};

use crate::hab_core::package::{PackageArchive, PackageIdent, PackageTarget};

use builder_core::http_client::{HttpClient, USER_AGENT_BLDR};
use tokio::io::AsyncWriteExt;
/// HTTP header name required by Artifactory for API key authentication.
const X_JFROG_ART_API: &str = "x-jfrog-art-api";

/// Async HTTP client for the Artifactory REST API.
///
/// Construct once via [`ArtifactoryClient::new`] and share across tasks
/// (it is `Clone` and backed by a connection-pooled [`HttpClient`]).
#[derive(Clone)]
pub struct ArtifactoryClient {
    inner: HttpClient,
    pub api_url: String,
    pub api_key: String,
    pub repo: String,
}

impl ArtifactoryClient {
    /// Construct a new client from the given configuration.
    ///
    /// Injects the `User-Agent` and `x-jfrog-art-api` headers into every
    /// request. Returns an error if:
    /// - `config.api_url` is not a valid base URL.
    /// - `config.api_key` contains characters that are not valid in an HTTP
    ///   header value (e.g. ASCII control characters).
    /// - `config.require_https` is `true` and `api_url` does not use the
    ///   `https://` scheme.
    pub fn new(config: ArtifactoryCfg) -> ArtifactoryResult<Self> {
        if config.require_https && !config.api_url.starts_with("https://") {
            return Err(ArtifactoryError::InvalidConfig(format!(
                "require_https is enabled but api_url does not use HTTPS: {}",
                config.api_url
            )));
        }
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT_BLDR.0.clone(), USER_AGENT_BLDR.1.clone());
        let api_key_header = HeaderValue::from_str(&config.api_key).map_err(|_| {
            ArtifactoryError::InvalidConfig(
                "api_key contains characters that are invalid in an HTTP header value".to_string(),
            )
        })?;
        headers.insert(HeaderName::from_static(X_JFROG_ART_API), api_key_header);

        Ok(ArtifactoryClient {
            inner: HttpClient::new(&config.api_url, headers)?,
            api_url: config.api_url,
            api_key: config.api_key,
            repo: config.repo,
        })
    }

    /// Upload a `.hart` file to Artifactory.
    ///
    /// Reads `source_path` from disk and HTTP-PUTs it to the path derived from
    /// `ident` and `target`. Returns the raw [`Response`] on success so callers
    /// can inspect Artifactory-specific headers if needed.
    pub async fn upload(
        &self,
        source_path: &Path,
        ident: &PackageIdent,
        target: PackageTarget,
    ) -> ArtifactoryResult<Response> {
        let url = self.url_path_for(ident, target);
        debug!("op=upload url={url}");

        let body: Body = tokio::fs::read(source_path)
            .await
            .map_err(ArtifactoryError::IO)?
            .into();

        let t = Instant::now();
        let resp = match self
            .inner
            .put(&url)
            .body(body)
            .send()
            .await
            .map_err(ArtifactoryError::HttpClient)
        {
            Ok(resp) => resp,
            Err(err) => {
                error!(
                    "op=upload status=error elapsed_ms={} url={} err={}",
                    t.elapsed().as_millis(),
                    url,
                    err
                );
                return Err(err);
            }
        };
        let elapsed_ms = t.elapsed().as_millis();

        if resp.status().is_success() {
            info!(
                "op=upload status=ok elapsed_ms={} http_status={}",
                elapsed_ms,
                resp.status().as_u16()
            );
            Ok(resp)
        } else {
            error!(
                "op=upload status=error elapsed_ms={} http_status={}",
                elapsed_ms,
                resp.status().as_u16()
            );
            Err(ArtifactoryError::ApiError(resp.status(), HashMap::new()))
        }
    }

    /// Download a `.hart` file from Artifactory and write it to `destination_path`.
    ///
    /// Streams the response body to avoid loading the entire archive into memory.
    /// Returns a [`PackageArchive`] handle pointing at the written file.
    pub async fn download(
        &self,
        destination_path: &Path,
        ident: &PackageIdent,
        target: PackageTarget,
    ) -> ArtifactoryResult<PackageArchive> {
        let url = self.url_path_for(ident, target);
        debug!("op=download url={url}");

        let t = Instant::now();
        let resp = match self
            .inner
            .get(&url)
            .send()
            .await
            .map_err(ArtifactoryError::HttpClient)
        {
            Ok(resp) => resp,
            Err(err) => {
                error!(
                    "op=download status=error elapsed_ms={} url={} err={}",
                    t.elapsed().as_millis(),
                    url,
                    err
                );
                return Err(err);
            }
        };

        if resp.status().is_success() {
            let mut file = tokio::fs::File::create(destination_path)
                .await
                .map_err(ArtifactoryError::IO)?;
            let mut stream = resp.bytes_stream();
            while let Some(chunk) = stream.next().await {
                file.write_all(&chunk?).await?;
            }
            let elapsed_ms = t.elapsed().as_millis();
            info!(
                "op=download status=ok elapsed_ms={elapsed_ms} dest={}",
                destination_path.display()
            );
            Ok(PackageArchive::new(destination_path)?)
        } else {
            let elapsed_ms = t.elapsed().as_millis();
            error!(
                "op=download status=error elapsed_ms={} http_status={}",
                elapsed_ms,
                resp.status().as_u16()
            );
            Err(ArtifactoryError::ApiError(resp.status(), HashMap::new()))
        }
    }

    /// Delete an artifact from Artifactory.
    ///
    /// Treats `404 Not Found` and `410 Gone` as success so that delete is
    /// idempotent — safe to call even if the artifact was already removed.
    pub async fn delete(
        &self,
        ident: &PackageIdent,
        target: PackageTarget,
    ) -> ArtifactoryResult<()> {
        let url = self.url_path_for(ident, target);
        debug!("op=delete url={url}");

        let t = Instant::now();
        let resp = match self
            .inner
            .delete(&url)
            .send()
            .await
            .map_err(ArtifactoryError::HttpClient)
        {
            Ok(resp) => resp,
            Err(err) => {
                error!(
                    "op=delete status=error elapsed_ms={} url={} err={}",
                    t.elapsed().as_millis(),
                    url,
                    err
                );
                return Err(err);
            }
        };
        let elapsed_ms = t.elapsed().as_millis();

        if resp.status().is_success() {
            info!(
                "op=delete status=ok elapsed_ms={} http_status={}",
                elapsed_ms,
                resp.status().as_u16()
            );
            Ok(())
        } else if resp.status() == reqwest::StatusCode::NOT_FOUND
            || resp.status() == reqwest::StatusCode::GONE
        {
            info!(
                "op=delete status=not_found elapsed_ms={} http_status={} note=already_removed",
                elapsed_ms,
                resp.status().as_u16()
            );
            Ok(())
        } else {
            error!(
                "op=delete status=error elapsed_ms={} http_status={}",
                elapsed_ms,
                resp.status().as_u16()
            );
            Err(ArtifactoryError::ApiError(resp.status(), HashMap::new()))
        }
    }

    /// Build the full Artifactory URL for a given package identity and target.
    ///
    /// Pattern: `<api_url>/artifactory/<repo>/<origin>/<name>/<version>/<release>/<target>/<hart>`
    ///
    /// # Panics
    /// Panics if `ident` is not fully qualified (missing version or release).
    fn url_path_for(&self, ident: &PackageIdent, target: PackageTarget) -> String {
        let hart_name = ident
            .archive_name_with_target(target)
            .expect("ident is fully qualified");

        format!(
            "{}/artifactory/{}/{}/{}/{}",
            self.api_url,
            self.repo,
            ident.iter().collect::<Vec<&str>>().join("/"),
            target.iter().collect::<Vec<&str>>().join("/"),
            hart_name
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ArtifactoryCfg;

    #[test]
    fn new_rejects_api_key_with_control_characters() {
        // A newline in an HTTP header value is invalid (potential header injection).
        // `new` must return Err rather than panic.
        let cfg = ArtifactoryCfg {
            api_key: "valid-prefix\ninvalid".to_string(),
            ..Default::default()
        };
        let result = ArtifactoryClient::new(cfg);
        assert!(
            result.is_err(),
            "expected Err for api_key containing a newline, got Ok"
        );
        // Confirm the error message is descriptive.
        let msg = format!("{}", result.err().unwrap());
        assert!(
            msg.contains("api_key"),
            "error message should mention api_key, got: {}",
            msg
        );
    }

    // --- require_https toggle tests -----------------------------------------

    /// Toggle OFF (default): HTTP URL must be accepted.
    #[test]
    fn require_https_off_accepts_http_url() {
        let cfg = ArtifactoryCfg {
            api_url: "http://artifactory.local:8081".to_string(),
            require_https: false,
            ..Default::default()
        };
        // new() validates config synchronously; no network call is made here.
        let result = ArtifactoryClient::new(cfg);
        assert!(
            result.is_ok(),
            "require_https=false should accept an HTTP URL, got: {:?}",
            result.err().map(|e| e.to_string())
        );
    }

    /// Toggle ON: HTTP URL must be rejected with a clear InvalidConfig error.
    #[test]
    fn require_https_on_rejects_http_url() {
        let cfg = ArtifactoryCfg {
            api_url: "http://artifactory.local:8081".to_string(),
            require_https: true,
            ..Default::default()
        };
        let result = ArtifactoryClient::new(cfg);
        assert!(
            result.is_err(),
            "require_https=true must reject an HTTP URL"
        );
        let msg = format!("{}", result.err().unwrap());
        assert!(
            msg.contains("require_https"),
            "error should mention require_https, got: {}",
            msg
        );
        assert!(
            msg.contains("http://"),
            "error should include the offending URL, got: {}",
            msg
        );
    }

    /// Toggle ON: HTTPS URL must be accepted.
    #[test]
    fn require_https_on_accepts_https_url() {
        let cfg = ArtifactoryCfg {
            api_url: "https://artifactory.example.com".to_string(),
            require_https: true,
            ..Default::default()
        };
        let result = ArtifactoryClient::new(cfg);
        assert!(
            result.is_ok(),
            "require_https=true should accept an HTTPS URL, got: {:?}",
            result.err().map(|e| e.to_string())
        );
    }
}
