use std::collections::{HashMap, HashSet};
use std::time::Duration;

use async_trait::async_trait;
use chrono::{Datelike, Local, Utc};
use log::debug;
use reqwest::header::{HeaderMap, RETRY_AFTER};
use reqwest::{Client, StatusCode};
use serde_json::Value;

use crate::domain::TmdbMovieDetails;
use crate::error::{AppError, AppResult};
use crate::logging::{preview_for_log, redact_secret};
use crate::retry::{RetryDirective, RetryPolicy, retry_with_backoff};

const REQUEST_TIMEOUT_SECONDS: u64 = 30;
const MAX_LOG_BODY_PREVIEW_CHARS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TmdbAuthentication {
    ApiKey,
    BearerToken,
}

#[async_trait]
pub trait TmdbService: Send + Sync {
    async fn get_movie_ratings_and_summaries(
        &self,
        movie_names: &[String],
        access_token: &str,
    ) -> AppResult<HashMap<String, TmdbMovieDetails>>;
}

#[derive(Clone)]
pub struct ReqwestTmdbClient {
    client: Client,
    auth_url: String,
    search_url: String,
    retry_policy: RetryPolicy,
}

impl ReqwestTmdbClient {
    pub fn new() -> AppResult<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECONDS))
            .build()
            .map_err(|error| AppError::Http(error.to_string()))?;
        Ok(Self {
            client,
            auth_url: "https://api.themoviedb.org/3/authentication".to_string(),
            search_url: "https://api.themoviedb.org/3/search/movie".to_string(),
            retry_policy: RetryPolicy::network_requests(),
        })
    }

    pub fn with_base_urls(
        auth_url: impl Into<String>,
        search_url: impl Into<String>,
    ) -> AppResult<Self> {
        let mut client = Self::new()?;
        client.auth_url = auth_url.into();
        client.search_url = search_url.into();
        Ok(client)
    }

    pub fn with_retry_policy(mut self, retry_policy: RetryPolicy) -> Self {
        self.retry_policy = retry_policy;
        self
    }

    async fn fetch_movie_details(&self, movie_name: &str, access_token: &str) -> AppResult<Value> {
        let authentication_mode = self.authentication_mode(access_token);

        retry_with_backoff(self.retry_policy, |attempt| {
            let redacted_token = redact_secret(access_token);
            async move {
                let search_params = self.make_search_params(movie_name);
                debug!(
                    "TMDB movie search request attempt={attempt} movie={movie_name:?} url={} auth_mode={authentication_mode:?} token={} params={search_params:?}",
                    self.search_url,
                    redacted_token,
            );
            let response = self
                .authorize_request(self.client.get(&self.search_url), access_token)
                .map_err(|error| {
                    debug!(
                        "TMDB movie search request setup failed attempt={attempt} movie={movie_name:?} url={} error={error}",
                        self.search_url,
                    );
                    RetryDirective::fail(error)
                })?
                .query(&search_params)
                .send()
                .await
                .map_err(|error| {
                    classify_tmdb_transport_error(
                        "movie search",
                        attempt,
                        &self.search_url,
                        Some(movie_name),
                        error,
                    )
                })?;
            let retry_after = retry_after_delay(response.headers());
            let status = response.status();
            debug!(
                "TMDB movie search response attempt={attempt} movie={movie_name:?} url={} status={status} retry_after={retry_after:?}",
                self.search_url,
            );

            if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                debug!(
                    "TMDB movie search authorization failed attempt={attempt} movie={movie_name:?} url={} body_preview={}",
                    self.search_url,
                    response_body_preview(response).await,
                );
                return Err(RetryDirective::fail(tmdb_status_error(status)));
            }

            if is_retryable_tmdb_status(status) {
                let error = tmdb_status_error(status);
                debug!(
                    "TMDB movie search received retryable status attempt={attempt} movie={movie_name:?} url={} status={status} body_preview={}",
                    self.search_url,
                    response_body_preview(response).await,
                );
                return Err(match retry_after {
                    Some(delay) => RetryDirective::retry_after(error, delay),
                    None => RetryDirective::retry(error),
                });
            }

            if status.is_client_error() || status.is_server_error() {
                debug!(
                    "TMDB movie search failed with non-retryable status attempt={attempt} movie={movie_name:?} url={} status={status} body_preview={}",
                    self.search_url,
                    response_body_preview(response).await,
                );
                return Err(RetryDirective::fail(tmdb_status_error(status)));
            }

            let body = response.text().await.map_err(|error| {
                classify_tmdb_response_read_error(
                    "movie search",
                    attempt,
                    &self.search_url,
                    Some(movie_name),
                    error,
                )
            })?;
            debug!(
                "TMDB movie search body received attempt={attempt} movie={movie_name:?} url={} bytes={}",
                self.search_url,
                body.len(),
            );
            serde_json::from_str::<Value>(&body).map_err(|error| {
                debug!(
                    "TMDB movie search JSON parse failed attempt={attempt} movie={movie_name:?} url={} error={error} body_preview={}",
                    self.search_url,
                    preview_for_log(&body, MAX_LOG_BODY_PREVIEW_CHARS),
                );
                RetryDirective::fail(AppError::Http(format!(
                    "TMDB returned invalid JSON for movie `{movie_name}`: {error}"
                )))
            })
            }
        })
        .await
    }

    async fn fetch_all_movie_details(
        &self,
        movie_names: &[String],
        access_token: &str,
    ) -> AppResult<HashMap<String, Value>> {
        let mut output = HashMap::new();
        let mut seen = HashSet::new();
        let mut last_non_auth_error = None;

        for movie_name in movie_names {
            if !seen.insert(movie_name.clone()) {
                continue;
            }
            match self.fetch_movie_details(movie_name, access_token).await {
                Ok(data) => {
                    output.insert(movie_name.clone(), data);
                }
                Err(AppError::Http(message))
                    if message.contains(&StatusCode::UNAUTHORIZED.to_string())
                        || message.contains(&StatusCode::FORBIDDEN.to_string()) =>
                {
                    debug!(
                        "TMDB movie details fetch aborted because authentication failed for movie={movie_name:?}: {message}"
                    );
                    return Err(AppError::Http(message));
                }
                Err(error) => {
                    debug!(
                        "TMDB movie details fetch failed for movie={movie_name:?}; falling back to null payload: {error}"
                    );
                    last_non_auth_error = Some(error.clone());
                    output.insert(movie_name.clone(), Value::Null);
                }
            }
        }

        if !output.is_empty() && output.values().all(Value::is_null) {
            debug!("TMDB movie details fetch failed for every requested movie");
            return Err(last_non_auth_error
                .unwrap_or_else(|| AppError::Http("All TMDB requests failed.".to_string())));
        }

        Ok(output)
    }

    pub async fn verify_api_key(&self, access_token: &str) -> bool {
        let authentication_mode = self.authentication_mode(access_token);

        retry_with_backoff(self.retry_policy, |attempt| {
            let redacted_token = redact_secret(access_token);
            async move {
                debug!(
                    "TMDB authentication request attempt={attempt} url={} auth_mode={authentication_mode:?} token={}",
                    self.auth_url,
                    redacted_token,
                );
            let request = match self.authorize_request(self.client.get(&self.auth_url), access_token)
            {
                Ok(request) => request,
                Err(error) => {
                    debug!(
                        "TMDB authentication request setup failed attempt={attempt} url={} error={error}",
                        self.auth_url,
                    );
                    self.client.get(&self.auth_url)
                }
            };
            let response = request.send().await.map_err(|error| {
                classify_tmdb_transport_error("authentication", attempt, &self.auth_url, None, error)
            })?;
            let retry_after = retry_after_delay(response.headers());
            let status = response.status();
            debug!(
                "TMDB authentication response attempt={attempt} url={} status={status} retry_after={retry_after:?}",
                self.auth_url,
            );

            if status == StatusCode::OK {
                return Ok(true);
            }

            if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                debug!(
                    "TMDB authentication rejected credentials attempt={attempt} url={} body_preview={}",
                    self.auth_url,
                    response_body_preview(response).await,
                );
                return Ok(false);
            }

            if is_retryable_tmdb_status(status) {
                let error = tmdb_status_error(status);
                debug!(
                    "TMDB authentication received retryable status attempt={attempt} url={} status={status} body_preview={}",
                    self.auth_url,
                    response_body_preview(response).await,
                );
                return Err(match retry_after {
                    Some(delay) => RetryDirective::retry_after(error, delay),
                    None => RetryDirective::retry(error),
                });
            }

            debug!(
                "TMDB authentication returned unexpected non-success status attempt={attempt} url={} status={status} body_preview={}",
                self.auth_url,
                response_body_preview(response).await,
                );
                Ok(false)
            }
        })
        .await
        .unwrap_or(false)
    }

    fn authorize_request(
        &self,
        request: reqwest::RequestBuilder,
        access_token: &str,
    ) -> AppResult<reqwest::RequestBuilder> {
        match self.authentication_mode(access_token) {
            TmdbAuthentication::ApiKey => Ok(request.query(&[("api_key", access_token)])),
            TmdbAuthentication::BearerToken => {
                Ok(request.headers(self.make_bearer_headers(access_token)?))
            }
        }
    }

    fn make_bearer_headers(&self, access_token: &str) -> AppResult<reqwest::header::HeaderMap> {
        let mut headers = reqwest::header::HeaderMap::new();
        let bearer = reqwest::header::HeaderValue::from_str(&format!("Bearer {access_token}"))
            .map_err(|error| AppError::Http(error.to_string()))?;
        headers.insert(
            reqwest::header::ACCEPT,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
        headers.insert(reqwest::header::AUTHORIZATION, bearer);
        Ok(headers)
    }

    fn authentication_mode(&self, access_token: &str) -> TmdbAuthentication {
        if looks_like_tmdb_v3_api_key(access_token) {
            TmdbAuthentication::ApiKey
        } else {
            TmdbAuthentication::BearerToken
        }
    }

    fn make_search_params(&self, movie_name: &str) -> [(&str, String); 5] {
        let current_year = Local::now().year();
        [
            ("query", movie_name.to_string()),
            ("include_adult", "true".to_string()),
            ("language", "pl-PL".to_string()),
            ("year", format!("{current_year},{}", current_year - 1)),
            ("page", "1".to_string()),
        ]
    }
}

fn looks_like_tmdb_v3_api_key(access_token: &str) -> bool {
    access_token.len() == 32 && access_token.chars().all(|character| character.is_ascii_hexdigit())
}

fn is_retryable_tmdb_status(status: StatusCode) -> bool {
    status == StatusCode::REQUEST_TIMEOUT
        || status == StatusCode::TOO_MANY_REQUESTS
        || status.is_server_error()
}

fn tmdb_status_error(status: StatusCode) -> AppError {
    AppError::Http(format!("TMDB request failed with status {status}."))
}

fn classify_tmdb_transport_error(
    operation: &str,
    attempt: usize,
    url: &str,
    movie_name: Option<&str>,
    error: reqwest::Error,
) -> RetryDirective<AppError> {
    let app_error = AppError::Http(error.to_string());
    let retryable =
        error.is_timeout() || error.is_connect() || error.is_request() || error.is_body();
    debug!(
        "TMDB {operation} transport error attempt={attempt} url={url} movie={movie_name:?} retryable={retryable} timeout={} connect={} request={} body={} error={error}",
        error.is_timeout(),
        error.is_connect(),
        error.is_request(),
        error.is_body(),
    );
    if retryable { RetryDirective::retry(app_error) } else { RetryDirective::fail(app_error) }
}

fn classify_tmdb_response_read_error(
    operation: &str,
    attempt: usize,
    url: &str,
    movie_name: Option<&str>,
    error: reqwest::Error,
) -> RetryDirective<AppError> {
    let app_error = AppError::Http(error.to_string());
    let retryable = error.is_timeout() || error.is_body();
    debug!(
        "TMDB {operation} response read error attempt={attempt} url={url} movie={movie_name:?} retryable={retryable} timeout={} body={} error={error}",
        error.is_timeout(),
        error.is_body(),
    );
    if retryable { RetryDirective::retry(app_error) } else { RetryDirective::fail(app_error) }
}

fn retry_after_delay(headers: &HeaderMap) -> Option<Duration> {
    let raw_value = headers.get(RETRY_AFTER)?;
    let raw_value = match raw_value.to_str() {
        Ok(raw_value) => raw_value,
        Err(error) => {
            debug!("TMDB response included an invalid Retry-After header: {error}");
            return None;
        }
    };

    if let Ok(delay_seconds) = raw_value.parse::<u64>() {
        return Some(Duration::from_secs(delay_seconds));
    }

    if let Ok(retry_after) = chrono::DateTime::parse_from_rfc2822(raw_value) {
        return Some(
            retry_after
                .with_timezone(&Utc)
                .signed_duration_since(Utc::now())
                .to_std()
                .unwrap_or(Duration::ZERO),
        );
    }

    debug!("TMDB response included an unparseable Retry-After header value={raw_value:?}");
    None
}

#[async_trait]
impl TmdbService for ReqwestTmdbClient {
    async fn get_movie_ratings_and_summaries(
        &self,
        movie_names: &[String],
        access_token: &str,
    ) -> AppResult<HashMap<String, TmdbMovieDetails>> {
        let movie_data = self.fetch_all_movie_details(movie_names, access_token).await?;
        let mut output = HashMap::new();
        for movie_name in movie_names {
            if let Some(data) = movie_data.get(movie_name) {
                output.insert(
                    movie_name.clone(),
                    TmdbMovieDetails {
                        rating: parse_movie_rating(data),
                        summary: parse_movie_summary(data),
                    },
                );
            }
        }
        Ok(output)
    }
}

pub fn ensure_single_result(movie_data: &Value) -> bool {
    movie_data.get("results").and_then(Value::as_array).is_some_and(|results| results.len() == 1)
}

pub fn parse_movie_rating(movie_data: &Value) -> String {
    if !ensure_single_result(movie_data) {
        debug!(
            "TMDB rating parser expected exactly one result but found results_count={:?}; payload={}",
            tmdb_results_count(movie_data),
            tmdb_payload_preview(movie_data),
        );
        return "0.0/10".to_string();
    }
    match (
        movie_data["results"][0].get("vote_average").and_then(Value::as_f64),
        movie_data["results"][0].get("vote_count").and_then(Value::as_i64),
    ) {
        (Some(vote_average), Some(vote_count)) => {
            format!("{vote_average}/10\n(głosy: {vote_count})")
        }
        _ => {
            debug!(
                "TMDB rating parser could not find both vote_average and vote_count; payload={}",
                tmdb_payload_preview(movie_data),
            );
            "0.0/10".to_string()
        }
    }
}

pub fn parse_movie_summary(movie_data: &Value) -> String {
    if !ensure_single_result(movie_data) {
        debug!(
            "TMDB summary parser expected exactly one result but found results_count={:?}; payload={}",
            tmdb_results_count(movie_data),
            tmdb_payload_preview(movie_data),
        );
        return "Brak opisu filmu.".to_string();
    }
    match movie_data["results"][0].get("overview").and_then(Value::as_str) {
        Some(overview) => overview.to_string(),
        None => {
            debug!(
                "TMDB summary parser could not find an overview field; payload={}",
                tmdb_payload_preview(movie_data),
            );
            "Brak opisu filmu.".to_string()
        }
    }
}

fn tmdb_results_count(movie_data: &Value) -> Option<usize> {
    movie_data.get("results").and_then(Value::as_array).map(Vec::len)
}

fn tmdb_payload_preview(movie_data: &Value) -> String {
    preview_for_log(&movie_data.to_string(), MAX_LOG_BODY_PREVIEW_CHARS)
}

async fn response_body_preview(response: reqwest::Response) -> String {
    match response.text().await {
        Ok(body) if body.trim().is_empty() => "<empty>".to_string(),
        Ok(body) => preview_for_log(&body, MAX_LOG_BODY_PREVIEW_CHARS),
        Err(error) => format!("<unavailable: {error}>"),
    }
}
