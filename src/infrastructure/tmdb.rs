use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use async_trait::async_trait;
use chrono::{Datelike, NaiveDate};
use futures::{StreamExt, stream};
use log::debug;
use regex::Regex;
use reqwest::{Client, StatusCode};
use serde::Deserialize;

use crate::cinema::common::parse_movie_page_fallback_details;
use crate::domain::{
    MovieLookupMetadata, MoviePageFallbackDetails, TmdbLookupMovie, TmdbMovieDetails,
};
use crate::error::{AppError, AppResult};
use crate::logging::{preview_for_log, redact_secret, response_body_preview};
use crate::retry::{RetryDirective, RetryPolicy, retry_after_delay, retry_with_backoff};

const REQUEST_TIMEOUT_SECONDS: u64 = 30;
const MAX_LOG_BODY_PREVIEW_CHARS: usize = 256;
const TMDB_LANGUAGE: &str = "pl-PL";
const TMDB_REGION: &str = "PL";
const TMDB_APPEND_TO_RESPONSE: &str = "credits";
const MAX_CONCURRENT_MOVIE_LOOKUPS: usize = 4;
const MAX_DETAILED_CANDIDATES: usize = 3;
const PRELIMINARY_WEAK_SCORE: i32 = 120;
const PRELIMINARY_MIN_GAP: i32 = 12;
const PRELIMINARY_CONFIDENT_SCORE: i32 = 138;
const PRELIMINARY_CONFIDENT_GAP: i32 = 18;
const FINAL_MIN_SCORE: i32 = 145;
const FINAL_MIN_GAP: i32 = 12;
const RELAXED_TITLE_MATCH_SCORE: i32 = 94;
const RELAXED_DETAILED_SUPPORT_SCORE: i32 = 18;
const RELAXED_DETAILED_MIN_GAP: i32 = 4;
const MAX_ACCEPTABLE_RUNTIME_CONFLICT_MINUTES: u16 = 25;
const MAX_ACCEPTABLE_YEAR_CONFLICT_YEARS: i32 = 2;
const CINEMA_CITY_ACCEPT_LANGUAGE: &str = "pl-PL,pl;q=0.9,en-US;q=0.8,en;q=0.7";
const CINEMA_CITY_BROWSER_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";

static TITLE_SUFFIX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?iu)(?:[\s\-–:|/()]+(?:(?:film\s+z\s+napisami|z\s+napisami|bez\s+napisów|bez\s+napisow|wersja\s+oryginalna)|(?:(?:pl|en|ua|ukr|polski|polska|angielski|angielska|ukraiński|ukrainski|ukraińska|ukrainska|polish|english|ukrainian)\s+)?(?:dubbing|dubbed|dubbingiem|lektor|napisy|napisami|subbed|subtitles?)))+$",
    )
    .expect("title suffix regex must compile")
});

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TmdbAuthentication {
    ApiKey,
    BearerToken,
}

#[async_trait]
pub trait TmdbService: Send + Sync {
    async fn get_movie_ratings_and_summaries(
        &self,
        movies: &[TmdbLookupMovie],
        access_token: &str,
    ) -> AppResult<HashMap<String, TmdbMovieDetails>>;
}

#[derive(Clone)]
pub struct ReqwestTmdbClient {
    client: Client,
    auth_url: String,
    search_url: String,
    movie_details_base_url: String,
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
            movie_details_base_url: "https://api.themoviedb.org/3/movie".to_string(),
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
        client.movie_details_base_url = derive_movie_details_base_url(&client.search_url);
        Ok(client)
    }

    pub fn with_retry_policy(mut self, retry_policy: RetryPolicy) -> Self {
        self.retry_policy = retry_policy;
        self
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
                let request =
                    match self.authorize_request(self.client.get(&self.auth_url), access_token) {
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
                    classify_tmdb_transport_error(
                        "authentication",
                        attempt,
                        &self.auth_url,
                        None,
                        error,
                    )
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
                        response_body_preview(response, MAX_LOG_BODY_PREVIEW_CHARS).await,
                    );
                    return Ok(false);
                }

                if is_retryable_tmdb_status(status) {
                    let error = tmdb_status_error(status);
                    debug!(
                        "TMDB authentication received retryable status attempt={attempt} url={} status={status} body_preview={}",
                        self.auth_url,
                        response_body_preview(response, MAX_LOG_BODY_PREVIEW_CHARS).await,
                    );
                    return Err(match retry_after {
                        Some(delay) => RetryDirective::retry_after(error, delay),
                        None => RetryDirective::retry(error),
                    });
                }

                debug!(
                    "TMDB authentication returned unexpected non-success status attempt={attempt} url={} status={status} body_preview={}",
                    self.auth_url,
                    response_body_preview(response, MAX_LOG_BODY_PREVIEW_CHARS).await,
                );
                Ok(false)
            }
        })
        .await
        .unwrap_or(false)
    }

    async fn search_movies(
        &self,
        search_request: &TmdbSearchRequest,
        access_token: &str,
    ) -> AppResult<TmdbSearchResponse> {
        let authentication_mode = self.authentication_mode(access_token);
        retry_with_backoff(self.retry_policy, |attempt| {
            let redacted_token = redact_secret(access_token);
            async move {
                let search_params = self.make_search_params(search_request);
                debug!(
                    "TMDB movie search request attempt={attempt} query={:?} year={:?} url={} auth_mode={authentication_mode:?} token={} params={search_params:?}",
                    search_request.query,
                    search_request.year,
                    self.search_url,
                    redacted_token,
                );
                let response = self
                    .authorize_request(self.client.get(&self.search_url), access_token)
                    .map_err(|error| {
                        debug!(
                            "TMDB movie search request setup failed attempt={attempt} query={:?} url={} error={error}",
                            search_request.query,
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
                            Some(search_request.query.as_str()),
                            error,
                        )
                    })?;
                let retry_after = retry_after_delay(response.headers());
                let status = response.status();
                debug!(
                    "TMDB movie search response attempt={attempt} query={:?} url={} status={status} retry_after={retry_after:?}",
                    search_request.query,
                    self.search_url,
                );

                if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                    debug!(
                        "TMDB movie search authorization failed attempt={attempt} query={:?} url={} body_preview={}",
                        search_request.query,
                        self.search_url,
                        response_body_preview(response, MAX_LOG_BODY_PREVIEW_CHARS).await,
                    );
                    return Err(RetryDirective::fail(tmdb_status_error(status)));
                }

                if is_retryable_tmdb_status(status) {
                    let error = tmdb_status_error(status);
                    debug!(
                        "TMDB movie search received retryable status attempt={attempt} query={:?} url={} status={status} body_preview={}",
                        search_request.query,
                        self.search_url,
                        response_body_preview(response, MAX_LOG_BODY_PREVIEW_CHARS).await,
                    );
                    return Err(match retry_after {
                        Some(delay) => RetryDirective::retry_after(error, delay),
                        None => RetryDirective::retry(error),
                    });
                }

                if status.is_client_error() || status.is_server_error() {
                    debug!(
                        "TMDB movie search failed with non-retryable status attempt={attempt} query={:?} url={} status={status} body_preview={}",
                        search_request.query,
                        self.search_url,
                        response_body_preview(response, MAX_LOG_BODY_PREVIEW_CHARS).await,
                    );
                    return Err(RetryDirective::fail(tmdb_status_error(status)));
                }

                let body = response.text().await.map_err(|error| {
                    classify_tmdb_response_read_error(
                        "movie search",
                        attempt,
                        &self.search_url,
                        Some(search_request.query.as_str()),
                        error,
                    )
                })?;
                debug!(
                    "TMDB movie search body received attempt={attempt} query={:?} url={} bytes={}",
                    search_request.query,
                    self.search_url,
                    body.len(),
                );
                serde_json::from_str::<TmdbSearchResponse>(&body).map_err(|error| {
                    debug!(
                        "TMDB movie search JSON parse failed attempt={attempt} query={:?} url={} error={error} body_preview={}",
                        search_request.query,
                        self.search_url,
                        preview_for_log(&body, MAX_LOG_BODY_PREVIEW_CHARS),
                    );
                    RetryDirective::fail(AppError::Http(format!(
                        "TMDB returned invalid JSON for search `{}`: {error}",
                        search_request.query
                    )))
                })
            }
        })
        .await
    }

    async fn fetch_movie_details_payload(
        &self,
        movie_id: u64,
        access_token: &str,
    ) -> AppResult<TmdbMovieDetailsResponse> {
        let details_url =
            format!("{}/{}", self.movie_details_base_url.trim_end_matches('/'), movie_id);
        let authentication_mode = self.authentication_mode(access_token);
        retry_with_backoff(self.retry_policy, |attempt| {
            let redacted_token = redact_secret(access_token);
            let details_url = details_url.clone();
            async move {
                let params = [
                    ("language", TMDB_LANGUAGE.to_string()),
                    ("append_to_response", TMDB_APPEND_TO_RESPONSE.to_string()),
                ];
                debug!(
                    "TMDB movie details request attempt={attempt} movie_id={} url={} auth_mode={authentication_mode:?} token={} params={params:?}",
                    movie_id,
                    details_url,
                    redacted_token,
                );
                let response = self
                    .authorize_request(self.client.get(&details_url), access_token)
                    .map_err(RetryDirective::fail)?
                    .query(&params)
                    .send()
                    .await
                    .map_err(|error| {
                        classify_tmdb_transport_error(
                            "movie details",
                            attempt,
                            &details_url,
                            None,
                            error,
                        )
                    })?;
                let retry_after = retry_after_delay(response.headers());
                let status = response.status();
                debug!(
                    "TMDB movie details response attempt={attempt} movie_id={} url={} status={status} retry_after={retry_after:?}",
                    movie_id,
                    details_url,
                );

                if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                    debug!(
                        "TMDB movie details authorization failed attempt={attempt} movie_id={} url={} body_preview={}",
                        movie_id,
                        details_url,
                        response_body_preview(response, MAX_LOG_BODY_PREVIEW_CHARS).await,
                    );
                    return Err(RetryDirective::fail(tmdb_status_error(status)));
                }

                if is_retryable_tmdb_status(status) {
                    let error = tmdb_status_error(status);
                    debug!(
                        "TMDB movie details received retryable status attempt={attempt} movie_id={} url={} status={status} body_preview={}",
                        movie_id,
                        details_url,
                        response_body_preview(response, MAX_LOG_BODY_PREVIEW_CHARS).await,
                    );
                    return Err(match retry_after {
                        Some(delay) => RetryDirective::retry_after(error, delay),
                        None => RetryDirective::retry(error),
                    });
                }

                if status.is_client_error() || status.is_server_error() {
                    debug!(
                        "TMDB movie details failed with non-retryable status attempt={attempt} movie_id={} url={} status={status} body_preview={}",
                        movie_id,
                        details_url,
                        response_body_preview(response, MAX_LOG_BODY_PREVIEW_CHARS).await,
                    );
                    return Err(RetryDirective::fail(tmdb_status_error(status)));
                }

                let body = response.text().await.map_err(|error| {
                    classify_tmdb_response_read_error(
                        "movie details",
                        attempt,
                        &details_url,
                        None,
                        error,
                    )
                })?;
                debug!(
                    "TMDB movie details body received attempt={attempt} movie_id={} url={} bytes={}",
                    movie_id,
                    details_url,
                    body.len(),
                );
                serde_json::from_str::<TmdbMovieDetailsResponse>(&body).map_err(|error| {
                    debug!(
                        "TMDB movie details JSON parse failed attempt={attempt} movie_id={} url={} error={error} body_preview={}",
                        movie_id,
                        details_url,
                        preview_for_log(&body, MAX_LOG_BODY_PREVIEW_CHARS),
                    );
                    RetryDirective::fail(AppError::Http(format!(
                        "TMDB returned invalid JSON for movie id `{movie_id}`: {error}"
                    )))
                })
            }
        })
        .await
    }

    async fn fetch_movie_page_fallback_details(
        &self,
        movie_page_url: &str,
    ) -> AppResult<MoviePageFallbackDetails> {
        retry_with_backoff(self.retry_policy, |attempt| async move {
            debug!(
                "Movie page fallback request attempt={attempt} url={movie_page_url}",
            );
            let response = self
                .client
                .get(movie_page_url)
                .header(reqwest::header::USER_AGENT, CINEMA_CITY_BROWSER_USER_AGENT)
                .header(
                    reqwest::header::ACCEPT,
                    "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
                )
                .header(reqwest::header::ACCEPT_LANGUAGE, CINEMA_CITY_ACCEPT_LANGUAGE)
                .send()
                .await
                .map_err(|error| {
                    let app_error = AppError::Http(format!(
                        "Nie udało się pobrać strony filmu `{movie_page_url}`: {error}"
                    ));
                    if error.is_timeout() || error.is_connect() || error.is_request() {
                        RetryDirective::retry(app_error)
                    } else {
                        RetryDirective::fail(app_error)
                    }
                })?;
            let status = response.status();
            if status.is_client_error() || status.is_server_error() {
                debug!(
                    "Movie page fallback request failed attempt={attempt} url={movie_page_url} status={status} body_preview={}",
                    response_body_preview(response, MAX_LOG_BODY_PREVIEW_CHARS).await,
                );
                return Err(RetryDirective::fail(AppError::Http(format!(
                    "Żądanie strony filmu zakończyło się błędem: status {status}."
                ))));
            }
            let body = response.text().await.map_err(|error| {
                let app_error = AppError::Http(format!(
                    "Nie udało się odczytać strony filmu `{movie_page_url}`: {error}"
                ));
                if error.is_timeout() || error.is_body() {
                    RetryDirective::retry(app_error)
                } else {
                    RetryDirective::fail(app_error)
                }
            })?;
            parse_movie_page_fallback_details(&body).map_err(RetryDirective::fail)
        })
        .await
    }

    async fn resolve_movie(
        &self,
        movie: &TmdbLookupMovie,
        access_token: &str,
        movie_page_cache: &mut HashMap<String, Option<MoviePageFallbackDetails>>,
        details_cache: &mut HashMap<u64, TmdbMovieDetailsResponse>,
    ) -> AppResult<Option<TmdbMovieDetails>> {
        let search_year = resolve_search_year(&movie.lookup_metadata);
        let mut attempted_searches = HashSet::<(String, Option<i32>)>::new();
        let mut candidates = Vec::new();
        let initial_queries = build_search_queries(movie, None);
        self.extend_search_results_for_queries(
            &initial_queries,
            None,
            access_token,
            &mut attempted_searches,
            &mut candidates,
        )
        .await?;
        let mut fallback_details = None;
        let mut ranked = rank_search_candidates(movie, fallback_details.as_ref(), &candidates);

        if search_year.is_some() && is_preliminary_weak(&ranked) {
            self.extend_search_results_for_queries(
                &initial_queries,
                search_year,
                access_token,
                &mut attempted_searches,
                &mut candidates,
            )
            .await?;
            ranked = rank_search_candidates(movie, fallback_details.as_ref(), &candidates);
        }

        if is_preliminary_weak(&ranked) {
            fallback_details = self.load_movie_page_details(movie, movie_page_cache).await;
            let fallback_queries = build_search_queries(movie, fallback_details.as_ref());
            self.extend_search_results_for_queries(
                &fallback_queries,
                None,
                access_token,
                &mut attempted_searches,
                &mut candidates,
            )
            .await?;
            ranked = rank_search_candidates(movie, fallback_details.as_ref(), &candidates);
            if search_year.is_some() && is_preliminary_weak(&ranked) {
                self.extend_search_results_for_queries(
                    &fallback_queries,
                    search_year,
                    access_token,
                    &mut attempted_searches,
                    &mut candidates,
                )
                .await?;
            }
        }

        ranked = rank_search_candidates(movie, fallback_details.as_ref(), &candidates);
        if ranked.is_empty() {
            debug!(
                "TMDB movie lookup produced no candidates lookup_key={:?} title={:?}",
                movie.lookup_key, movie.title,
            );
            return Ok(None);
        }

        let detailed_candidates =
            self.fetch_detailed_candidates(&ranked, access_token, details_cache).await;
        if !detailed_candidates.is_empty() {
            let final_candidates =
                rank_detailed_candidates(movie, fallback_details.as_ref(), &detailed_candidates);
            if is_final_confident(movie, fallback_details.as_ref(), &final_candidates) {
                return Ok(Some(render_details_from_detailed(&final_candidates[0].details)));
            }
        }

        if is_preliminary_confident(movie, fallback_details.as_ref(), &ranked) {
            return Ok(Some(render_details_from_search(&ranked[0].result)));
        }

        debug!(
            "TMDB movie lookup left movie unmatched because confidence stayed low lookup_key={:?} title={:?} top_score={} gap={}",
            movie.lookup_key,
            movie.title,
            ranked[0].score,
            score_gap(&ranked),
        );
        Ok(None)
    }

    async fn load_movie_page_details(
        &self,
        movie: &TmdbLookupMovie,
        movie_page_cache: &mut HashMap<String, Option<MoviePageFallbackDetails>>,
    ) -> Option<MoviePageFallbackDetails> {
        let movie_page_url = movie.lookup_metadata.movie_page_url.as_deref()?;
        if let Some(cached) = movie_page_cache.get(movie_page_url) {
            return cached.clone();
        }
        if !movie_page_url.contains("cinema-city.pl") {
            movie_page_cache.insert(movie_page_url.to_string(), None);
            return None;
        }

        let details = match self.fetch_movie_page_fallback_details(movie_page_url).await {
            Ok(details) => Some(details),
            Err(error) => {
                debug!(
                    "Movie page fallback lookup failed lookup_key={:?} movie_page_url={movie_page_url} error={error}",
                    movie.lookup_key,
                );
                None
            }
        };
        movie_page_cache.insert(movie_page_url.to_string(), details.clone());
        details
    }

    async fn fetch_detailed_candidates(
        &self,
        ranked_candidates: &[RankedSearchCandidate],
        access_token: &str,
        details_cache: &mut HashMap<u64, TmdbMovieDetailsResponse>,
    ) -> Vec<TmdbDetailedCandidate> {
        let mut detailed_candidates = Vec::new();
        for ranked_candidate in ranked_candidates.iter().take(MAX_DETAILED_CANDIDATES) {
            let details = if let Some(cached) = details_cache.get(&ranked_candidate.result.id) {
                cached.clone()
            } else {
                match self
                    .fetch_movie_details_payload(ranked_candidate.result.id, access_token)
                    .await
                {
                    Ok(details) => {
                        details_cache.insert(ranked_candidate.result.id, details.clone());
                        details
                    }
                    Err(error) => {
                        debug!(
                            "TMDB movie details enrichment failed movie_id={} error={error}",
                            ranked_candidate.result.id,
                        );
                        continue;
                    }
                }
            };
            detailed_candidates
                .push(TmdbDetailedCandidate { details, preliminary_score: ranked_candidate.score });
        }
        detailed_candidates
    }

    async fn extend_search_results_for_queries(
        &self,
        queries: &[String],
        year: Option<i32>,
        access_token: &str,
        attempted_searches: &mut HashSet<(String, Option<i32>)>,
        candidates: &mut Vec<TmdbSearchResult>,
    ) -> AppResult<()> {
        for query in queries {
            let normalized_query = normalize_for_comparison(query);
            if normalized_query.is_empty() || !attempted_searches.insert((normalized_query, year)) {
                continue;
            }
            candidates.extend(
                self.search_movies(&TmdbSearchRequest { query: query.clone(), year }, access_token)
                    .await?
                    .results,
            );
        }
        dedupe_search_results(candidates);
        Ok(())
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

    fn make_search_params(&self, search_request: &TmdbSearchRequest) -> Vec<(&str, String)> {
        let mut params = vec![
            ("query", search_request.query.clone()),
            ("include_adult", "true".to_string()),
            ("language", TMDB_LANGUAGE.to_string()),
            ("region", TMDB_REGION.to_string()),
            ("page", "1".to_string()),
        ];
        if let Some(year) = search_request.year {
            params.push(("primary_release_year", year.to_string()));
        }
        params
    }
}

#[async_trait]
impl TmdbService for ReqwestTmdbClient {
    async fn get_movie_ratings_and_summaries(
        &self,
        movies: &[TmdbLookupMovie],
        access_token: &str,
    ) -> AppResult<HashMap<String, TmdbMovieDetails>> {
        let mut seen = HashSet::new();
        let unique_movies = movies
            .iter()
            .filter(|movie| seen.insert(movie.lookup_key.clone()))
            .cloned()
            .collect::<Vec<_>>();
        debug!(
            "TMDB batch lookup starting requested_movies={} unique_movies={} concurrency_limit={MAX_CONCURRENT_MOVIE_LOOKUPS}",
            movies.len(),
            unique_movies.len(),
        );

        let lookup_results = stream::iter(unique_movies.into_iter().map(|movie| async move {
            let mut movie_page_cache = HashMap::<String, Option<MoviePageFallbackDetails>>::new();
            let mut details_cache = HashMap::<u64, TmdbMovieDetailsResponse>::new();
            let result = self
                .resolve_movie(&movie, access_token, &mut movie_page_cache, &mut details_cache)
                .await;
            (movie.lookup_key.clone(), movie.title.clone(), result)
        }))
        .buffer_unordered(MAX_CONCURRENT_MOVIE_LOOKUPS)
        .collect::<Vec<_>>()
        .await;

        let mut output = HashMap::new();
        let mut last_non_auth_error = None;
        let mut authorization_error = None;
        let mut successful_lookups = 0_usize;

        for (lookup_key, title, result) in lookup_results {
            match result {
                Ok(Some(details)) => {
                    successful_lookups += 1;
                    output.insert(lookup_key, details);
                }
                Ok(None) => {
                    successful_lookups += 1;
                    output.insert(
                        lookup_key,
                        TmdbMovieDetails { rating: String::new(), summary: String::new() },
                    );
                }
                Err(AppError::Http(message))
                    if message.contains(&StatusCode::UNAUTHORIZED.to_string())
                        || message.contains(&StatusCode::FORBIDDEN.to_string()) =>
                {
                    authorization_error.get_or_insert(AppError::Http(message));
                }
                Err(error) => {
                    debug!(
                        "TMDB movie lookup failed lookup_key={:?} title={:?}; leaving blank details: {error}",
                        lookup_key, title,
                    );
                    last_non_auth_error = Some(error.clone());
                    output.insert(
                        lookup_key,
                        TmdbMovieDetails { rating: String::new(), summary: String::new() },
                    );
                }
            }
        }

        if let Some(error) = authorization_error {
            return Err(error);
        }

        if successful_lookups == 0 {
            return Err(last_non_auth_error
                .unwrap_or_else(|| AppError::Http("All TMDB requests failed.".to_string())));
        }

        Ok(output)
    }
}

#[derive(Debug, Clone)]
struct TmdbSearchRequest {
    query: String,
    year: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
struct TmdbSearchResponse {
    #[serde(default)]
    results: Vec<TmdbSearchResult>,
}

#[derive(Debug, Clone, Deserialize)]
struct TmdbSearchResult {
    id: u64,
    title: Option<String>,
    #[serde(rename = "original_title")]
    original_title: Option<String>,
    #[serde(rename = "original_language")]
    original_language: Option<String>,
    #[serde(rename = "release_date")]
    release_date: Option<String>,
    #[serde(default)]
    vote_average: Option<f64>,
    #[serde(default)]
    vote_count: Option<u32>,
    overview: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct TmdbMovieDetailsResponse {
    title: Option<String>,
    #[serde(rename = "original_title")]
    original_title: Option<String>,
    #[serde(rename = "original_language")]
    original_language: Option<String>,
    #[serde(rename = "release_date")]
    release_date: Option<String>,
    runtime: Option<u16>,
    #[serde(default)]
    vote_average: Option<f64>,
    #[serde(default)]
    vote_count: Option<u32>,
    overview: Option<String>,
    #[serde(default)]
    genres: Vec<TmdbGenre>,
    #[serde(rename = "production_countries", default)]
    production_countries: Vec<TmdbProductionCountry>,
    credits: Option<TmdbCredits>,
}

#[derive(Debug, Clone, Deserialize)]
struct TmdbGenre {
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct TmdbProductionCountry {
    #[serde(rename = "iso_3166_1")]
    iso_3166_1: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct TmdbCredits {
    #[serde(default)]
    cast: Vec<TmdbCastMember>,
    #[serde(default)]
    crew: Vec<TmdbCrewMember>,
}

#[derive(Debug, Clone, Deserialize)]
struct TmdbCastMember {
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct TmdbCrewMember {
    job: Option<String>,
    name: String,
}

#[derive(Debug, Clone)]
struct RankedSearchCandidate {
    result: TmdbSearchResult,
    score: i32,
}

#[derive(Debug, Clone)]
struct TmdbDetailedCandidate {
    details: TmdbMovieDetailsResponse,
    preliminary_score: i32,
}

#[derive(Debug, Clone)]
struct RankedDetailedCandidate {
    details: TmdbMovieDetailsResponse,
    score: i32,
}

#[derive(Debug, Clone)]
struct TitleVariants {
    normalized: Vec<String>,
}

fn derive_movie_details_base_url(search_url: &str) -> String {
    if let Some((prefix, _)) = search_url.rsplit_once("/search/movie") {
        return format!("{prefix}/movie");
    }
    format!("{}/movie", search_url.trim_end_matches('/'))
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
    subject: Option<&str>,
    error: reqwest::Error,
) -> RetryDirective<AppError> {
    let app_error = AppError::Http(error.to_string());
    let retryable =
        error.is_timeout() || error.is_connect() || error.is_request() || error.is_body();
    debug!(
        "TMDB {operation} transport error attempt={attempt} url={url} subject={subject:?} retryable={retryable} timeout={} connect={} request={} body={} error={error}",
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
    subject: Option<&str>,
    error: reqwest::Error,
) -> RetryDirective<AppError> {
    let app_error = AppError::Http(error.to_string());
    let retryable = error.is_timeout() || error.is_body();
    debug!(
        "TMDB {operation} response read error attempt={attempt} url={url} subject={subject:?} retryable={retryable} timeout={} body={} error={error}",
        error.is_timeout(),
        error.is_body(),
    );
    if retryable { RetryDirective::retry(app_error) } else { RetryDirective::fail(app_error) }
}

fn rank_search_candidates(
    movie: &TmdbLookupMovie,
    fallback_details: Option<&MoviePageFallbackDetails>,
    candidates: &[TmdbSearchResult],
) -> Vec<RankedSearchCandidate> {
    let title_variants = build_title_variants(movie, fallback_details);
    let mut ranked = candidates
        .iter()
        .cloned()
        .map(|candidate| RankedSearchCandidate {
            score: score_preliminary_candidate(&movie.lookup_metadata, &title_variants, &candidate),
            result: candidate,
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        compare_rank(left.score, left.result.vote_count, right.score, right.result.vote_count)
    });
    ranked
}

fn rank_detailed_candidates(
    movie: &TmdbLookupMovie,
    fallback_details: Option<&MoviePageFallbackDetails>,
    candidates: &[TmdbDetailedCandidate],
) -> Vec<RankedDetailedCandidate> {
    let title_variants = build_title_variants(movie, fallback_details);
    let mut ranked = candidates
        .iter()
        .cloned()
        .map(|candidate| RankedDetailedCandidate {
            score: score_detailed_candidate(
                &movie.lookup_metadata,
                fallback_details,
                &title_variants,
                &candidate.details,
                candidate.preliminary_score,
            ),
            details: candidate.details,
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        compare_rank(left.score, left.details.vote_count, right.score, right.details.vote_count)
    });
    ranked
}

fn compare_rank(
    left_score: i32,
    left_votes: Option<u32>,
    right_score: i32,
    right_votes: Option<u32>,
) -> std::cmp::Ordering {
    right_score
        .cmp(&left_score)
        .then_with(|| right_votes.unwrap_or_default().cmp(&left_votes.unwrap_or_default()))
}

fn score_preliminary_candidate(
    metadata: &MovieLookupMetadata,
    title_variants: &TitleVariants,
    candidate: &TmdbSearchResult,
) -> i32 {
    title_score(title_variants, &[candidate.title.as_deref(), candidate.original_title.as_deref()])
        + year_score(resolve_search_year(metadata), candidate.release_date.as_deref())
        + language_score(
            metadata.original_language_code.as_deref(),
            candidate.original_language.as_deref(),
        )
        + vote_score(candidate.vote_count)
}

fn score_detailed_candidate(
    metadata: &MovieLookupMetadata,
    fallback_details: Option<&MoviePageFallbackDetails>,
    title_variants: &TitleVariants,
    candidate: &TmdbMovieDetailsResponse,
    preliminary_score: i32,
) -> i32 {
    title_score(title_variants, &[candidate.title.as_deref(), candidate.original_title.as_deref()])
        + year_score(resolve_search_year(metadata), candidate.release_date.as_deref())
        + language_score(
            metadata.original_language_code.as_deref(),
            candidate.original_language.as_deref(),
        )
        + runtime_score(metadata.runtime_minutes, candidate.runtime)
        + genre_score(&metadata.genre_tags, &candidate.genres)
        + country_score(
            fallback_details.and_then(|details| details.country.as_deref()),
            &candidate.production_countries,
        )
        + director_score(fallback_details.map(|details| details.directors.as_slice()), candidate)
        + cast_score(fallback_details.map(|details| details.cast.as_slice()), candidate)
        + (preliminary_score / 10)
        + vote_score(candidate.vote_count)
}

fn build_search_queries(
    movie: &TmdbLookupMovie,
    fallback_details: Option<&MoviePageFallbackDetails>,
) -> Vec<String> {
    let mut queries = Vec::new();
    push_search_query(&mut queries, &movie.title);
    for alternate_title in &movie.lookup_metadata.alternate_titles {
        push_search_query(&mut queries, alternate_title);
    }
    if let Some(original_title) =
        fallback_details.and_then(|details| details.original_title.as_deref())
    {
        push_search_query(&mut queries, original_title);
    }
    queries
}

fn push_search_query(queries: &mut Vec<String>, title: &str) {
    let query = build_search_query(title);
    let normalized_query = normalize_for_comparison(&query);
    if normalized_query.is_empty()
        || queries
            .iter()
            .any(|existing_query| normalize_for_comparison(existing_query) == normalized_query)
    {
        return;
    }
    queries.push(query);
}

fn build_title_variants(
    movie: &TmdbLookupMovie,
    fallback_details: Option<&MoviePageFallbackDetails>,
) -> TitleVariants {
    let mut normalized = Vec::new();
    push_title_variants(&mut normalized, &movie.title);
    for alternate_title in &movie.lookup_metadata.alternate_titles {
        push_title_variants(&mut normalized, alternate_title);
    }
    if let Some(original_title) =
        fallback_details.and_then(|details| details.original_title.as_deref())
    {
        push_title_variants(&mut normalized, original_title);
    }
    TitleVariants { normalized }
}

fn push_title_variants(variants: &mut Vec<String>, title: &str) {
    for candidate in [title.trim().to_string(), build_search_query(title)] {
        let normalized_candidate = normalize_for_comparison(&candidate);
        if normalized_candidate.is_empty() || variants.contains(&normalized_candidate) {
            continue;
        }
        variants.push(normalized_candidate);
    }
}

fn title_score(title_variants: &TitleVariants, candidate_titles: &[Option<&str>]) -> i32 {
    candidate_titles
        .iter()
        .flatten()
        .map(|candidate_title| score_single_title(title_variants, candidate_title))
        .max()
        .unwrap_or_default()
}

fn score_single_title(title_variants: &TitleVariants, candidate_title: &str) -> i32 {
    let normalized_candidate = normalize_for_comparison(candidate_title);
    if normalized_candidate.is_empty() {
        return 0;
    }

    title_variants
        .normalized
        .iter()
        .map(|variant| score_title_variant(variant, &normalized_candidate, 100, 82, 68, 56, 40))
        .max()
        .unwrap_or_default()
}

fn score_title_variant(
    variant: &str,
    candidate: &str,
    exact_score: i32,
    token_score: i32,
    contains_score: i32,
    high_similarity_score: i32,
    medium_similarity_score: i32,
) -> i32 {
    if variant.is_empty() || candidate.is_empty() {
        return 0;
    }
    if variant == candidate {
        return exact_score;
    }
    if token_set(variant) == token_set(candidate) {
        return token_score;
    }
    if candidate.contains(variant) || variant.contains(candidate) {
        return contains_score;
    }
    let similarity = token_similarity(variant, candidate);
    if similarity >= 0.8 {
        return high_similarity_score;
    }
    if similarity >= 0.6 {
        return medium_similarity_score;
    }
    0
}

fn year_score(expected_year: Option<i32>, release_date: Option<&str>) -> i32 {
    let Some(expected_year) = expected_year else {
        return 0;
    };
    let Some(candidate_year) = release_date.and_then(parse_release_year) else {
        return 0;
    };
    match (candidate_year - expected_year).abs() {
        0 => 20,
        1 => 10,
        2 => 5,
        _ => 0,
    }
}

fn language_score(expected_language: Option<&str>, candidate_language: Option<&str>) -> i32 {
    let Some(expected_language) = expected_language.map(normalize_language_code) else {
        return 0;
    };
    let Some(candidate_language) = candidate_language.map(normalize_language_code) else {
        return 0;
    };
    if expected_language == candidate_language { 12 } else { 0 }
}

fn runtime_score(expected_runtime: Option<u16>, candidate_runtime: Option<u16>) -> i32 {
    let (Some(expected_runtime), Some(candidate_runtime)) = (expected_runtime, candidate_runtime)
    else {
        return 0;
    };
    let difference = expected_runtime.abs_diff(candidate_runtime);
    match difference {
        0 => 18,
        1..=5 => 12,
        6..=10 => 6,
        _ => 0,
    }
}

fn genre_score(expected_genres: &[String], candidate_genres: &[TmdbGenre]) -> i32 {
    if expected_genres.is_empty() || candidate_genres.is_empty() {
        return 0;
    }
    let expected = expected_genres.iter().cloned().collect::<HashSet<_>>();
    let overlaps = candidate_genres
        .iter()
        .map(|genre| normalize_for_comparison(&genre.name))
        .filter(|genre| expected.contains(genre))
        .count() as i32;
    (overlaps * 8).min(16)
}

fn country_score(
    expected_country: Option<&str>,
    candidate_countries: &[TmdbProductionCountry],
) -> i32 {
    let Some(expected_country) = expected_country else {
        return 0;
    };
    let normalized_expected = normalize_for_comparison(expected_country);
    let uppercase_expected = expected_country.trim().to_ascii_uppercase();
    for candidate_country in candidate_countries {
        if candidate_country
            .iso_3166_1
            .as_deref()
            .is_some_and(|code| code.eq_ignore_ascii_case(&uppercase_expected))
        {
            return 10;
        }
        if candidate_country
            .name
            .as_deref()
            .map(normalize_for_comparison)
            .is_some_and(|name| name == normalized_expected)
        {
            return 10;
        }
    }
    0
}

fn director_score(
    expected_directors: Option<&[String]>,
    candidate: &TmdbMovieDetailsResponse,
) -> i32 {
    let Some(expected_directors) = expected_directors else {
        return 0;
    };
    let actual_directors = candidate
        .credits
        .as_ref()
        .map(|credits| {
            credits
                .crew
                .iter()
                .filter(|member| {
                    member.job.as_deref().is_some_and(|job| job.eq_ignore_ascii_case("Director"))
                })
                .map(|member| normalize_for_comparison(&member.name))
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    if actual_directors.is_empty() {
        return 0;
    }
    let overlaps = expected_directors
        .iter()
        .map(|director| normalize_for_comparison(director))
        .filter(|director| actual_directors.contains(director))
        .count();
    match overlaps {
        0 => 0,
        1 => 14,
        _ => 18,
    }
}

fn cast_score(expected_cast: Option<&[String]>, candidate: &TmdbMovieDetailsResponse) -> i32 {
    let Some(expected_cast) = expected_cast else {
        return 0;
    };
    let actual_cast = candidate
        .credits
        .as_ref()
        .map(|credits| {
            credits
                .cast
                .iter()
                .map(|member| normalize_for_comparison(&member.name))
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    if actual_cast.is_empty() {
        return 0;
    }
    let overlaps = expected_cast
        .iter()
        .map(|cast_member| normalize_for_comparison(cast_member))
        .filter(|cast_member| actual_cast.contains(cast_member))
        .count();
    match overlaps {
        0 => 0,
        1 => 10,
        _ => 14,
    }
}

fn vote_score(vote_count: Option<u32>) -> i32 {
    ((vote_count.unwrap_or_default() / 1000) as i32).min(8)
}

fn resolve_search_year(metadata: &MovieLookupMetadata) -> Option<i32> {
    metadata
        .production_year
        .or_else(|| metadata.polish_premiere_date.as_deref().and_then(parse_release_year))
}

fn build_search_query(title: &str) -> String {
    let stripped_title = TITLE_SUFFIX_RE.replace(title, "");
    let stripped = stripped_title.trim().trim_matches(|character: char| {
        matches!(character, '-' | '–' | ':' | '|' | '/' | '(' | ')' | '[' | ']')
    });
    if stripped.is_empty() { title.trim().to_string() } else { stripped.to_string() }
}

fn parse_release_year(release_date: &str) -> Option<i32> {
    NaiveDate::parse_from_str(release_date, "%Y-%m-%d").ok().map(|date| date.year())
}

fn normalize_for_comparison(value: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_separator = false;
    for character in value.chars() {
        let lowered = fold_character(character).to_ascii_lowercase();
        if lowered.is_ascii_alphanumeric() {
            normalized.push(lowered);
            previous_was_separator = false;
        } else if !previous_was_separator {
            normalized.push(' ');
            previous_was_separator = true;
        }
    }
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_language_code(value: &str) -> String {
    normalize_for_comparison(value)
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase()
}

fn fold_character(character: char) -> char {
    match character {
        'ą' | 'á' | 'à' | 'ä' | 'â' => 'a',
        'ć' | 'č' => 'c',
        'ę' | 'é' | 'è' | 'ë' | 'ê' => 'e',
        'ł' => 'l',
        'ń' => 'n',
        'ó' | 'ö' | 'ô' | 'ò' => 'o',
        'ś' | 'š' => 's',
        'ź' | 'ż' | 'ž' => 'z',
        'Ą' | 'Á' | 'À' | 'Ä' | 'Â' => 'A',
        'Ć' | 'Č' => 'C',
        'Ę' | 'É' | 'È' | 'Ë' | 'Ê' => 'E',
        'Ł' => 'L',
        'Ń' => 'N',
        'Ó' | 'Ö' | 'Ô' | 'Ò' => 'O',
        'Ś' | 'Š' => 'S',
        'Ź' | 'Ż' | 'Ž' => 'Z',
        _ => character,
    }
}

fn token_set(value: &str) -> HashSet<String> {
    value.split_whitespace().filter(|token| !token.is_empty()).map(str::to_string).collect()
}

fn token_similarity(left: &str, right: &str) -> f32 {
    let left_tokens = token_set(left);
    let right_tokens = token_set(right);
    if left_tokens.is_empty() || right_tokens.is_empty() {
        return 0.0;
    }
    let intersection = left_tokens.intersection(&right_tokens).count() as f32;
    let union = left_tokens.union(&right_tokens).count() as f32;
    if union == 0.0 { 0.0 } else { intersection / union }
}

fn dedupe_search_results(results: &mut Vec<TmdbSearchResult>) {
    let mut seen = HashSet::new();
    results.retain(|candidate| seen.insert(candidate.id));
}

fn score_gap<T>(ranked_candidates: &[T]) -> i32
where
    T: RankedScore,
{
    match ranked_candidates {
        [] => 0,
        [_] => i32::MAX,
        [top, second, ..] => top.score() - second.score(),
    }
}

fn is_preliminary_weak(ranked_candidates: &[RankedSearchCandidate]) -> bool {
    ranked_candidates.is_empty()
        || ranked_candidates[0].score < PRELIMINARY_WEAK_SCORE
        || score_gap(ranked_candidates) < PRELIMINARY_MIN_GAP
}

fn is_preliminary_confident(
    movie: &TmdbLookupMovie,
    fallback_details: Option<&MoviePageFallbackDetails>,
    ranked_candidates: &[RankedSearchCandidate],
) -> bool {
    (!ranked_candidates.is_empty()
        && ranked_candidates[0].score >= PRELIMINARY_CONFIDENT_SCORE
        && score_gap(ranked_candidates) >= PRELIMINARY_CONFIDENT_GAP)
        || is_relaxed_preliminary_match(movie, fallback_details, ranked_candidates)
}

fn is_final_confident(
    movie: &TmdbLookupMovie,
    fallback_details: Option<&MoviePageFallbackDetails>,
    ranked_candidates: &[RankedDetailedCandidate],
) -> bool {
    (!ranked_candidates.is_empty()
        && ranked_candidates[0].score >= FINAL_MIN_SCORE
        && score_gap(ranked_candidates) >= FINAL_MIN_GAP)
        || is_relaxed_detailed_match(movie, fallback_details, ranked_candidates)
}

fn is_relaxed_preliminary_match(
    movie: &TmdbLookupMovie,
    fallback_details: Option<&MoviePageFallbackDetails>,
    ranked_candidates: &[RankedSearchCandidate],
) -> bool {
    let Some(top_candidate) = ranked_candidates.first() else {
        return false;
    };

    let title_variants = build_title_variants(movie, fallback_details);
    preliminary_title_match_score(&title_variants, &top_candidate.result)
        >= RELAXED_TITLE_MATCH_SCORE
        && !has_search_metadata_conflict(&movie.lookup_metadata, &top_candidate.result)
        && ranked_candidates.len() == 1
}

fn is_relaxed_detailed_match(
    movie: &TmdbLookupMovie,
    fallback_details: Option<&MoviePageFallbackDetails>,
    ranked_candidates: &[RankedDetailedCandidate],
) -> bool {
    let Some(top_candidate) = ranked_candidates.first() else {
        return false;
    };

    let title_variants = build_title_variants(movie, fallback_details);
    detailed_title_match_score(&title_variants, &top_candidate.details) >= RELAXED_TITLE_MATCH_SCORE
        && !has_detailed_metadata_conflict(&movie.lookup_metadata, &top_candidate.details)
        && (ranked_candidates.len() == 1
            || score_gap(ranked_candidates) >= RELAXED_DETAILED_MIN_GAP
            || detailed_metadata_support(
                &movie.lookup_metadata,
                fallback_details,
                &top_candidate.details,
            ) >= RELAXED_DETAILED_SUPPORT_SCORE)
}

fn render_details_from_search(movie: &TmdbSearchResult) -> TmdbMovieDetails {
    TmdbMovieDetails {
        rating: render_rating(movie.vote_average, movie.vote_count),
        summary: movie.overview.clone().unwrap_or_default(),
    }
}

fn render_details_from_detailed(movie: &TmdbMovieDetailsResponse) -> TmdbMovieDetails {
    TmdbMovieDetails {
        rating: render_rating(movie.vote_average, movie.vote_count),
        summary: movie.overview.clone().unwrap_or_default(),
    }
}

fn render_rating(vote_average: Option<f64>, vote_count: Option<u32>) -> String {
    match (vote_average, vote_count) {
        (Some(vote_average), Some(vote_count)) => {
            format!("{vote_average:.1}/10\n(głosy: {vote_count})")
        }
        _ => String::new(),
    }
}

trait RankedScore {
    fn score(&self) -> i32;
}

impl RankedScore for RankedSearchCandidate {
    fn score(&self) -> i32 {
        self.score
    }
}

impl RankedScore for RankedDetailedCandidate {
    fn score(&self) -> i32 {
        self.score
    }
}

fn preliminary_title_match_score(
    title_variants: &TitleVariants,
    candidate: &TmdbSearchResult,
) -> i32 {
    title_score(title_variants, &[candidate.title.as_deref(), candidate.original_title.as_deref()])
}

fn detailed_title_match_score(
    title_variants: &TitleVariants,
    candidate: &TmdbMovieDetailsResponse,
) -> i32 {
    title_score(title_variants, &[candidate.title.as_deref(), candidate.original_title.as_deref()])
}

fn detailed_metadata_support(
    metadata: &MovieLookupMetadata,
    fallback_details: Option<&MoviePageFallbackDetails>,
    candidate: &TmdbMovieDetailsResponse,
) -> i32 {
    year_score(resolve_search_year(metadata), candidate.release_date.as_deref())
        + language_score(
            metadata.original_language_code.as_deref(),
            candidate.original_language.as_deref(),
        )
        + runtime_score(metadata.runtime_minutes, candidate.runtime)
        + genre_score(&metadata.genre_tags, &candidate.genres)
        + country_score(
            fallback_details.and_then(|details| details.country.as_deref()),
            &candidate.production_countries,
        )
        + director_score(fallback_details.map(|details| details.directors.as_slice()), candidate)
        + cast_score(fallback_details.map(|details| details.cast.as_slice()), candidate)
}

fn has_search_metadata_conflict(
    metadata: &MovieLookupMetadata,
    candidate: &TmdbSearchResult,
) -> bool {
    has_year_conflict(resolve_search_year(metadata), candidate.release_date.as_deref())
        || has_language_conflict(
            metadata.original_language_code.as_deref(),
            candidate.original_language.as_deref(),
        )
}

fn has_detailed_metadata_conflict(
    metadata: &MovieLookupMetadata,
    candidate: &TmdbMovieDetailsResponse,
) -> bool {
    has_year_conflict(resolve_search_year(metadata), candidate.release_date.as_deref())
        || has_language_conflict(
            metadata.original_language_code.as_deref(),
            candidate.original_language.as_deref(),
        )
        || has_runtime_conflict(metadata.runtime_minutes, candidate.runtime)
}

fn has_year_conflict(expected_year: Option<i32>, release_date: Option<&str>) -> bool {
    let Some(expected_year) = expected_year else {
        return false;
    };
    let Some(candidate_year) = release_date.and_then(parse_release_year) else {
        return false;
    };
    (candidate_year - expected_year).abs() > MAX_ACCEPTABLE_YEAR_CONFLICT_YEARS
}

fn has_language_conflict(
    expected_language: Option<&str>,
    candidate_language: Option<&str>,
) -> bool {
    let Some(expected_language) = expected_language.map(normalize_language_code) else {
        return false;
    };
    let Some(candidate_language) = candidate_language.map(normalize_language_code) else {
        return false;
    };
    expected_language != candidate_language
}

fn has_runtime_conflict(expected_runtime: Option<u16>, candidate_runtime: Option<u16>) -> bool {
    let (Some(expected_runtime), Some(candidate_runtime)) = (expected_runtime, candidate_runtime)
    else {
        return false;
    };
    expected_runtime.abs_diff(candidate_runtime) > MAX_ACCEPTABLE_RUNTIME_CONFLICT_MINUTES
}

#[cfg(test)]
mod tests {
    use super::render_rating;

    #[test]
    fn render_rating_rounds_to_one_decimal_place() {
        assert_eq!(render_rating(Some(6.717), Some(184)), "6.7/10\n(głosy: 184)");
    }
}
