use std::collections::{HashMap, HashSet};
use std::time::Duration;

use async_trait::async_trait;
use chrono::{Datelike, Local, Utc};
use reqwest::header::{HeaderMap, RETRY_AFTER};
use reqwest::{Client, StatusCode};
use serde_json::Value;

use crate::domain::TmdbMovieDetails;
use crate::error::{AppError, AppResult};
use crate::retry::{RetryDirective, RetryPolicy, retry_with_backoff};

const REQUEST_TIMEOUT_SECONDS: u64 = 30;

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
        retry_with_backoff(self.retry_policy, |_| async {
            let response = self
                .authorize_request(self.client.get(&self.search_url), access_token)
                .map_err(RetryDirective::fail)?
                .query(&self.make_search_params(movie_name))
                .send()
                .await
                .map_err(classify_tmdb_transport_error)?;
            let retry_after = retry_after_delay(response.headers());
            let status = response.status();

            if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                return Err(RetryDirective::fail(tmdb_status_error(status)));
            }

            if is_retryable_tmdb_status(status) {
                let error = tmdb_status_error(status);
                return Err(match retry_after {
                    Some(delay) => RetryDirective::retry_after(error, delay),
                    None => RetryDirective::retry(error),
                });
            }

            if status.is_client_error() || status.is_server_error() {
                return Err(RetryDirective::fail(tmdb_status_error(status)));
            }

            response.json::<Value>().await.map_err(classify_tmdb_response_error)
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
                    return Err(AppError::Http(message));
                }
                Err(_) => {
                    output.insert(movie_name.clone(), Value::Null);
                }
            }
        }

        if !output.is_empty() && output.values().all(Value::is_null) {
            return Err(AppError::Http("All TMDB requests failed.".to_string()));
        }

        Ok(output)
    }

    pub async fn verify_api_key(&self, access_token: &str) -> bool {
        retry_with_backoff(self.retry_policy, |_| async {
            let request = self
                .authorize_request(self.client.get(&self.auth_url), access_token)
                .unwrap_or_else(|_| self.client.get(&self.auth_url));
            let response = request.send().await.map_err(classify_tmdb_transport_error)?;
            let retry_after = retry_after_delay(response.headers());
            let status = response.status();

            if status == StatusCode::OK {
                return Ok(true);
            }

            if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                return Ok(false);
            }

            if is_retryable_tmdb_status(status) {
                let error = tmdb_status_error(status);
                return Err(match retry_after {
                    Some(delay) => RetryDirective::retry_after(error, delay),
                    None => RetryDirective::retry(error),
                });
            }

            Ok(false)
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

fn classify_tmdb_transport_error(error: reqwest::Error) -> RetryDirective<AppError> {
    let app_error = AppError::Http(error.to_string());
    if error.is_timeout() || error.is_connect() || error.is_request() || error.is_body() {
        RetryDirective::retry(app_error)
    } else {
        RetryDirective::fail(app_error)
    }
}

fn classify_tmdb_response_error(error: reqwest::Error) -> RetryDirective<AppError> {
    let app_error = AppError::Http(error.to_string());
    if error.is_timeout() || error.is_body() {
        RetryDirective::retry(app_error)
    } else {
        RetryDirective::fail(app_error)
    }
}

fn retry_after_delay(headers: &HeaderMap) -> Option<Duration> {
    let raw_value = headers.get(RETRY_AFTER)?.to_str().ok()?;

    raw_value.parse::<u64>().ok().map(Duration::from_secs).or_else(|| {
        chrono::DateTime::parse_from_rfc2822(raw_value).ok().map(|retry_after| {
            retry_after
                .with_timezone(&Utc)
                .signed_duration_since(Utc::now())
                .to_std()
                .unwrap_or(Duration::ZERO)
        })
    })
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
        return "0.0/10".to_string();
    }
    match (
        movie_data["results"][0].get("vote_average").and_then(Value::as_f64),
        movie_data["results"][0].get("vote_count").and_then(Value::as_i64),
    ) {
        (Some(vote_average), Some(vote_count)) => {
            format!("{vote_average}/10\n(głosy: {vote_count})")
        }
        _ => "0.0/10".to_string(),
    }
}

pub fn parse_movie_summary(movie_data: &Value) -> String {
    if !ensure_single_result(movie_data) {
        return "Brak opisu filmu.".to_string();
    }
    movie_data["results"][0]
        .get("overview")
        .and_then(Value::as_str)
        .unwrap_or("Brak opisu filmu.")
        .to_string()
}
