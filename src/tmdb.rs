use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use chrono::{Datelike, Local};
use reqwest::{Client, StatusCode};
use serde_json::Value;

use crate::domain::TmdbMovieDetails;
use crate::error::{AppError, AppResult};

const REQUEST_TIMEOUT_SECONDS: u64 = 30;

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

    async fn fetch_movie_details(&self, movie_name: &str, access_token: &str) -> AppResult<Value> {
        let response = self
            .client
            .get(&self.search_url)
            .query(&self.make_search_params(movie_name))
            .headers(self.make_headers(access_token)?)
            .send()
            .await
            .map_err(|error| AppError::Http(error.to_string()))?;
        if response.status().is_client_error() || response.status().is_server_error() {
            return Err(AppError::Http(format!(
                "TMDB request failed with status {}.",
                response.status()
            )));
        }
        response.json::<Value>().await.map_err(|error| AppError::Http(error.to_string()))
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
        match self
            .client
            .get(&self.auth_url)
            .headers(match self.make_headers(access_token) {
                Ok(headers) => headers,
                Err(_) => return false,
            })
            .send()
            .await
        {
            Ok(response) => response.status() == StatusCode::OK,
            Err(_) => false,
        }
    }

    fn make_headers(&self, access_token: &str) -> AppResult<reqwest::header::HeaderMap> {
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
