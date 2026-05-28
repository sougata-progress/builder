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

use std::{collections::HashMap, path::Path};

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
    /// request. Returns an error if `config.api_url` is not a valid base URL.
    pub fn new(config: ArtifactoryCfg) -> ArtifactoryResult<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT_BLDR.0.clone(), USER_AGENT_BLDR.1.clone());
        headers.insert(
            HeaderName::from_static(X_JFROG_ART_API),
            HeaderValue::from_str(&config.api_key).expect("Invalid API key value"),
        );

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
        debug!(
            "ArtifactoryClient upload request for file path: {:?}",
            source_path
        );

        let url = self.url_path_for(ident, target);
        debug!("ArtifactoryClient upload url = {}", url);

        let body: Body = tokio::fs::read(source_path)
            .await
            .map_err(ArtifactoryError::IO)?
            .into();

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
                error!("ArtifactoryClient upload failed, err={}", err);
                return Err(err);
            }
        };

        debug!("Artifactory response status: {:?}", resp.status());

        if resp.status().is_success() {
            Ok(resp)
        } else {
            error!("Artifactory upload non-success status: {:?}", resp.status());
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
        debug!(
            "ArtifactoryClient download request for {} ({}) to destination path: {:?}",
            ident, target, destination_path
        );

        let url = self.url_path_for(ident, target);
        debug!("ArtifactoryClient download url = {}", url);

        let resp = match self
            .inner
            .get(&url)
            .send()
            .await
            .map_err(ArtifactoryError::HttpClient)
        {
            Ok(resp) => resp,
            Err(err) => {
                error!("ArtifactoryClient download failed, err={}", err);
                return Err(err);
            }
        };

        debug!("Artifactory response status: {:?}", resp.status());

        if resp.status().is_success() {
            let mut file = tokio::fs::File::create(destination_path)
                .await
                .map_err(ArtifactoryError::IO)?;
            let mut stream = resp.bytes_stream();
            while let Some(chunk) = stream.next().await {
                file.write_all(&chunk?).await?;
            }
            Ok(PackageArchive::new(destination_path)?)
        } else {
            error!(
                "Artifactory download non-success status: {:?}",
                resp.status()
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
        debug!("ArtifactoryClient delete url = {}", url);

        let resp = match self
            .inner
            .delete(&url)
            .send()
            .await
            .map_err(ArtifactoryError::HttpClient)
        {
            Ok(resp) => resp,
            Err(err) => {
                error!("ArtifactoryClient delete failed, err={}", err);
                return Err(err);
            }
        };

        debug!("Artifactory delete response status: {:?}", resp.status());

        if resp.status().is_success() {
            Ok(())
        } else if resp.status() == reqwest::StatusCode::NOT_FOUND
            || resp.status() == reqwest::StatusCode::GONE
        {
            warn!(
                "Artifactory delete returned {} for {} ({}); artifact may have already been \
                   removed",
                resp.status(),
                ident,
                target
            );
            Ok(())
        } else {
            error!("Artifactory delete non-success status: {:?}", resp.status());
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
