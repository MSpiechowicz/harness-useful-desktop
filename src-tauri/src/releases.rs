//! The HTTP side of the companion release lookup (`ecosystem_releases`).
//!
//! Each request is a `GET` of one fixed URL from
//! [`release_endpoint`](harness_business::ecosystem::release_endpoint) over HTTPS, with only a
//! `User-Agent` (GitHub refuses requests without one) and an `Accept` header: no credentials, no
//! cookies, no local data. Redirects are followed only to HTTPS URLs on the same host and port 443. A
//! connection gets 10 s, the whole request 15 s, and at most
//! [`MAX_RELEASE_BYTES`] of the body are read. Any failure yields `None`; it is logged without
//! the URL.

use std::time::Duration;

use harness_business::ecosystem::releases::{MAX_RELEASE_BYTES, RELEASE_HOST};
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue};
use reqwest::redirect::{Attempt, Policy};
use reqwest::{Client, StatusCode, Url};

/// Longest wait for a connection to GitHub.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Longest wait for a whole request, body included.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Most redirects followed (GitHub redirects a renamed repository once).
const MAX_REDIRECTS: usize = 3;

/// The media type GitHub's REST API documents for its JSON responses.
const GITHUB_JSON: &str = "application/vnd.github+json";

/// A client for the release lookups of app version `version`; `None` when the TLS stack cannot
/// be set up (logged).
pub fn release_client(version: &str) -> Option<Client> {
    // The updater installs the same provider before its first request; whichever runs first wins.
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }

    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static(GITHUB_JSON));

    let built = Client::builder()
        .https_only(true)
        .user_agent(format!("harness-desktop/{version}"))
        .default_headers(headers)
        .referer(false)
        .redirect(Policy::custom(same_host_redirect))
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build();

    built
        .inspect_err(|error| tracing::warn!(error = error_kind(error), "cannot set up release lookups"))
        .ok()
}

/// The body of a `200` response to `GET url`, at most [`MAX_RELEASE_BYTES`] long; `None` on any
/// failure.
pub async fn fetch_release(client: &Client, url: &'static str) -> Option<Vec<u8>> {
    let response = client
        .get(url)
        .send()
        .await
        .inspect_err(|error| tracing::debug!(error = error_kind(error), "release lookup failed"))
        .ok()?;

    let status = response.status();
    if status != StatusCode::OK {
        // 403 and 429 are GitHub's rate limits; they are retried after the failure cache expires.
        tracing::debug!(%status, "release lookup answered without a release");
        return None;
    }

    read_capped(response).await
}

/// Whether a redirect to `url` may be followed after `followed` earlier redirects: HTTPS on
/// [`RELEASE_HOST`], port 443, and fewer than [`MAX_REDIRECTS`] followed so far.
fn may_follow(url: &Url, followed: usize) -> bool {
    followed < MAX_REDIRECTS
        && url.scheme() == "https"
        && url.host_str() == Some(RELEASE_HOST)
        && url.port_or_known_default() == Some(443)
}

fn same_host_redirect(attempt: Attempt) -> reqwest::redirect::Action {
    // `previous` lists every URL already requested, the original one included.
    let followed = attempt.previous().len().saturating_sub(1);

    if may_follow(attempt.url(), followed) {
        attempt.follow()
    } else {
        // The redirect response itself comes back, and its status is not a `200`.
        attempt.stop()
    }
}

async fn read_capped(mut response: reqwest::Response) -> Option<Vec<u8>> {
    let announced = response.content_length().unwrap_or(0);
    if announced > MAX_RELEASE_BYTES as u64 {
        tracing::debug!(bytes = announced, "release response too large");
        return None;
    }

    let mut body = Vec::new();

    loop {
        let chunk = response
            .chunk()
            .await
            .inspect_err(|error| tracing::debug!(error = error_kind(error), "release response broke off"))
            .ok()?;

        let Some(chunk) = chunk else {
            return Some(body);
        };

        if body.len() + chunk.len() > MAX_RELEASE_BYTES {
            tracing::debug!("release response too large");
            return None;
        }

        body.extend_from_slice(&chunk);
    }
}

/// What went wrong, without the URL a `reqwest::Error` would print.
fn error_kind(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "timed out"
    } else if error.is_connect() {
        "cannot connect"
    } else if error.is_redirect() {
        "redirect refused"
    } else if error.is_body() || error.is_decode() {
        "bad body"
    } else if error.is_builder() {
        "bad client setup"
    } else {
        "request failed"
    }
}

#[cfg(test)]
mod tests {
    use harness_business::ecosystem::{ECOSYSTEM, release_endpoint};

    use super::*;

    fn url(text: &str) -> Url {
        Url::parse(text).expect("valid test URL")
    }

    #[test]
    fn redirects_are_followed_only_over_https_to_the_api_host() {
        let moved = url("https://api.github.com/repositories/42/releases/latest");

        assert!(may_follow(&moved, 0));
        assert!(may_follow(&moved, MAX_REDIRECTS - 1));
        assert!(!may_follow(&moved, MAX_REDIRECTS));

        assert!(!may_follow(
            &url("http://api.github.com/repositories/42/releases/latest"),
            0
        ));
        assert!(!may_follow(
            &url("https://github.com/MSpiechowicz/harness-useful-skills"),
            0
        ));
        assert!(!may_follow(
            &url("https://api.github.com.example.org/releases/latest"),
            0
        ));
        assert!(!may_follow(&url("https://example.org/?next=api.github.com"), 0));
    }

    #[test]
    fn redirects_to_another_port_are_not_followed() {
        assert!(may_follow(
            &url("https://api.github.com:443/repositories/42/releases/latest"),
            0
        ));

        assert!(!may_follow(
            &url("https://api.github.com:8443/repositories/42/releases/latest"),
            0
        ));
        assert!(!may_follow(
            &url("https://api.github.com:80/repositories/42/releases/latest"),
            0
        ));
    }

    #[test]
    fn every_release_url_is_one_the_client_would_request() {
        for id in ECOSYSTEM {
            assert!(may_follow(&url(release_endpoint(id)), 0), "{id:?}");
        }
    }

    #[test]
    fn the_client_builds_without_any_request() {
        assert!(release_client("0.1.0").is_some());
    }
}
