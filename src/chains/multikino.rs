use std::collections::HashSet;

use async_trait::async_trait;
use log::debug;
use reqwest::{Client, StatusCode};
use serde::Deserialize;

use crate::cinema::common::{MISSING_DATA_LABEL, normalize_lookup_text};
use crate::cinema::registry::CinemaChainClient;
use crate::domain::{
    CinemaChainId, CinemaVenue, MovieLookupMetadata, MoviePlayDetails, MoviePlayTime, Repertoire,
};
use crate::error::{AppError, AppResult};
use crate::logging::{preview_for_log, response_body_preview};
use crate::retry::{RetryDirective, RetryPolicy, retry_after_delay, retry_with_backoff};

pub const DEFAULT_MULTIKINO_BASE_URL: &str = "https://www.multikino.pl";

const DEFAULT_MULTIKINO_SHOWINGS_API_BASE_URL: &str =
    "https://www.multikino.pl/api/microservice/showings";
const MULTIKINO_ACCEPT_HEADER: &str = "application/json, text/plain, */*";
const MULTIKINO_BROWSER_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
const MULTIKINO_FILMS_QUERY: &str =
    "minEmbargoLevel=3&includesSession=true&includeSessionAttributes=true";
const MULTIKINO_ATTRIBUTE_GROUPS_QUERY: &str = "minEmbargoLevel=2";
const MAX_LOG_BODY_PREVIEW_CHARS: usize = 256;
const FORMAT_GROUP_NAME: &str = "Rodzaj pokazu";
const LANGUAGE_GROUP_NAME: &str = "Wersja językowa";
const IGNORED_SESSION_ATTRIBUTES: [&str; 2] = ["Single Seat", "SUPERHIT"];

#[derive(Clone)]
pub struct Multikino {
    base_url: String,
    showings_api_base_url: String,
    http_client: Client,
    retry_policy: RetryPolicy,
}

impl Multikino {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            showings_api_base_url: DEFAULT_MULTIKINO_SHOWINGS_API_BASE_URL.to_string(),
            http_client: Client::new(),
            retry_policy: RetryPolicy::network_requests(),
        }
    }

    pub fn with_retry_policy(mut self, retry_policy: RetryPolicy) -> Self {
        self.retry_policy = retry_policy;
        self
    }

    pub fn with_showings_api_base_url(mut self, showings_api_base_url: impl Into<String>) -> Self {
        self.showings_api_base_url = showings_api_base_url.into();
        self
    }

    fn venues_url(&self) -> String {
        format!("{}/cinemas", self.showings_api_base_url.trim_end_matches('/'))
    }

    fn films_url(&self, venue_id: &str) -> String {
        format!(
            "{}/cinemas/{}/films?{MULTIKINO_FILMS_QUERY}",
            self.showings_api_base_url.trim_end_matches('/'),
            venue_id.trim(),
        )
    }

    fn attribute_groups_url(&self, venue_id: &str) -> String {
        format!(
            "{}/attributes/showingAttributeGroups?cinemaId={}&{MULTIKINO_ATTRIBUTE_GROUPS_QUERY}",
            self.showings_api_base_url.trim_end_matches('/'),
            venue_id.trim(),
        )
    }

    async fn fetch_api_result<T>(&self, url: &str, operation: &str) -> AppResult<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        retry_with_backoff(self.retry_policy, |attempt| async move {
            debug!("Multikino API request attempt={attempt} operation={operation} url={url}");
            let response = self
                .http_client
                .get(url)
                .header(reqwest::header::USER_AGENT, MULTIKINO_BROWSER_USER_AGENT)
                .header(reqwest::header::ACCEPT, MULTIKINO_ACCEPT_HEADER)
                .send()
                .await
                .map_err(|error| {
                    let app_error = AppError::Http(format!(
                        "Nie udało się pobrać danych z API Multikino podczas {operation}: {error}"
                    ));
                    if error.is_timeout() || error.is_connect() {
                        RetryDirective::retry(app_error)
                    } else {
                        RetryDirective::fail(app_error)
                    }
                })?;
            let status = response.status();
            let retry_after = if status == StatusCode::TOO_MANY_REQUESTS
                || status == StatusCode::SERVICE_UNAVAILABLE
            {
                retry_after_delay(response.headers())
            } else {
                None
            };
            if status.is_server_error()
                || status == StatusCode::TOO_MANY_REQUESTS
                || status == StatusCode::REQUEST_TIMEOUT
            {
                let body_preview =
                    response_body_preview(response, MAX_LOG_BODY_PREVIEW_CHARS).await;
                debug!(
                    "Multikino API retryable response operation={operation} url={url} status={status} retry_after={retry_after:?} body_preview={body_preview}",
                );
                let error = AppError::Http(format!(
                    "API Multikino zwróciło błąd podczas {operation}: status {status}"
                ));
                return Err(match retry_after {
                    Some(delay) => RetryDirective::retry_after(error, delay),
                    None => RetryDirective::retry(error),
                });
            }
            if status.is_client_error() {
                let body_preview =
                    response_body_preview(response, MAX_LOG_BODY_PREVIEW_CHARS).await;
                debug!(
                    "Multikino API non-retryable response operation={operation} url={url} status={status} body_preview={body_preview}",
                );
                return Err(RetryDirective::fail(AppError::Http(format!(
                    "API Multikino zwróciło błąd podczas {operation}: status {status}"
                ))));
            }

            let body = response.text().await.map_err(|error| {
                let app_error = AppError::Http(format!(
                    "Nie udało się odczytać odpowiedzi z API Multikino podczas {operation}: {error}"
                ));
                if error.is_timeout() {
                    RetryDirective::retry(app_error)
                } else {
                    RetryDirective::fail(app_error)
                }
            })?;
            debug!(
                "Multikino API response received operation={operation} url={url} bytes={}",
                body.len(),
            );

            let payload = serde_json::from_str::<MultikinoApiResponse<T>>(&body).map_err(|error| {
                debug!(
                    "Multikino API JSON parse failed operation={operation} url={url} error={error} body_preview={}",
                    preview_for_log(&body, MAX_LOG_BODY_PREVIEW_CHARS),
                );
                RetryDirective::fail(AppError::Http(format!(
                    "Nie udało się odczytać odpowiedzi z API Multikino podczas {operation}: {error}"
                )))
            })?;

            payload.into_result(operation).map_err(RetryDirective::fail)
        })
        .await
    }

    fn parse_venues(groups: Vec<MultikinoCinemaGroup>) -> Vec<CinemaVenue> {
        let mut seen_ids = HashSet::new();
        let mut venues = Vec::new();

        for group in groups {
            for cinema in group.cinemas {
                let venue_id = cinema.cinema_id.trim();
                if venue_id.is_empty() || !seen_ids.insert(venue_id.to_string()) {
                    continue;
                }

                let venue_name =
                    cinema.full_name.as_deref().unwrap_or(&cinema.cinema_name).trim().to_string();
                if venue_name.is_empty() {
                    continue;
                }

                venues.push(CinemaVenue {
                    chain_id: CinemaChainId::Multikino.as_str().to_string(),
                    venue_id: venue_id.to_string(),
                    venue_name,
                });
            }
        }

        venues.sort_by(|left, right| left.venue_name.cmp(&right.venue_name));
        venues
    }

    fn extract_attribute_rules(groups: &[MultikinoShowingAttributeGroup]) -> AttributeRules {
        let mut rules = AttributeRules::default();

        for group in groups {
            let target = match group.name.as_str() {
                FORMAT_GROUP_NAME => Some(&mut rules.format_names),
                LANGUAGE_GROUP_NAME => Some(&mut rules.language_names),
                _ => None,
            };
            let Some(target) = target else {
                continue;
            };

            for attribute in &group.showing_attributes {
                let name = attribute.name.trim();
                if !name.is_empty() {
                    target.insert(name.to_string());
                }
            }
        }

        rules
    }

    fn parse_repertoire(
        &self,
        date: &str,
        films: Vec<MultikinoFilm>,
        attribute_rules: &AttributeRules,
    ) -> Vec<Repertoire> {
        films
            .iter()
            .filter_map(|film| self.parse_film_repertoire(date, film, attribute_rules))
            .collect()
    }

    fn parse_film_repertoire(
        &self,
        date: &str,
        film: &MultikinoFilm,
        attribute_rules: &AttributeRules,
    ) -> Option<Repertoire> {
        let mut sessions_for_date = film
            .showing_groups
            .iter()
            .filter(|group| normalize_api_date(&group.date).is_some_and(|value| value == date))
            .flat_map(|group| group.sessions.iter())
            .collect::<Vec<_>>();
        if sessions_for_date.is_empty() {
            return None;
        }
        sessions_for_date.sort_by(|left, right| left.start_time.cmp(&right.start_time));

        let mut play_details = Vec::<MoviePlayDetails>::new();
        for session in sessions_for_date {
            let Some(play_time_value) = extract_showtime_value(&session.start_time) else {
                continue;
            };
            let format = select_session_format(&session.attributes, attribute_rules);
            let play_language = select_session_language(&session.attributes, attribute_rules);
            let play_time = MoviePlayTime {
                value: play_time_value,
                url: booking_url_for_session(&self.base_url, session),
            };

            if let Some(existing) = play_details
                .iter_mut()
                .find(|detail| detail.format == format && detail.play_language == play_language)
            {
                existing.play_times.push(play_time);
            } else {
                play_details.push(MoviePlayDetails {
                    format,
                    play_language,
                    play_times: vec![play_time],
                });
            }
        }

        if play_details.is_empty() {
            return None;
        }

        Some(Repertoire {
            title: film.film_title.clone(),
            genres: join_or_missing(&film.genres),
            play_length: render_play_length(film.running_time, film.is_duration_unknown),
            original_language: MISSING_DATA_LABEL.to_string(),
            play_details,
            lookup_metadata: build_lookup_metadata(&self.base_url, film),
        })
    }
}

#[async_trait]
impl CinemaChainClient for Multikino {
    async fn fetch_repertoire(
        &self,
        date: &str,
        venue: &CinemaVenue,
    ) -> AppResult<Vec<Repertoire>> {
        let films_url = self.films_url(&venue.venue_id);
        debug!(
            "Multikino repertoire fetch starting url={films_url} venue_id={} venue_name={:?} date={date}",
            venue.venue_id, venue.venue_name,
        );
        let films = self
            .fetch_api_result::<Vec<MultikinoFilm>>(&films_url, "pobierania repertuaru")
            .await?;
        let attribute_rules = match self
            .fetch_api_result::<Vec<MultikinoShowingAttributeGroup>>(
                &self.attribute_groups_url(&venue.venue_id),
                "pobierania atrybutów repertuaru",
            )
            .await
        {
            Ok(groups) => Self::extract_attribute_rules(&groups),
            Err(error) => {
                debug!(
                    "Multikino attribute groups lookup failed venue_id={} date={date} error={error}",
                    venue.venue_id,
                );
                AttributeRules::default()
            }
        };

        Ok(self.parse_repertoire(date, films, &attribute_rules))
    }

    async fn fetch_venues(&self) -> AppResult<Vec<CinemaVenue>> {
        let url = self.venues_url();
        debug!("Multikino venues fetch starting url={url}");
        let groups = self
            .fetch_api_result::<Vec<MultikinoCinemaGroup>>(&url, "pobierania listy lokali")
            .await?;
        Ok(Self::parse_venues(groups))
    }
}

#[derive(Debug, Deserialize)]
struct MultikinoApiResponse<T> {
    result: T,
    #[serde(rename = "responseCode")]
    response_code: i32,
    #[serde(rename = "errorMessage")]
    error_message: Option<String>,
}

impl<T> MultikinoApiResponse<T> {
    fn into_result(self, operation: &str) -> AppResult<T> {
        if self.response_code == 0 {
            Ok(self.result)
        } else {
            Err(AppError::Http(format!(
                "API Multikino zwróciło błąd podczas {operation}: {}",
                self.error_message.unwrap_or_else(|| format!("kod {}", self.response_code))
            )))
        }
    }
}

#[derive(Debug, Deserialize)]
struct MultikinoCinemaGroup {
    cinemas: Vec<MultikinoCinema>,
}

#[derive(Debug, Deserialize)]
struct MultikinoCinema {
    #[serde(rename = "cinemaId")]
    cinema_id: String,
    #[serde(rename = "cinemaName")]
    cinema_name: String,
    #[serde(rename = "fullName")]
    full_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MultikinoFilm {
    #[serde(rename = "filmId")]
    film_id: String,
    #[serde(rename = "filmUrl")]
    film_url: Option<String>,
    #[serde(rename = "filmTitle")]
    film_title: String,
    #[serde(rename = "originalTitle")]
    original_title: Option<String>,
    #[serde(rename = "releaseDate")]
    release_date: Option<String>,
    #[serde(rename = "runningTime", default)]
    running_time: u16,
    #[serde(rename = "isDurationUnknown", default)]
    is_duration_unknown: bool,
    #[serde(default)]
    genres: Vec<String>,
    #[serde(rename = "showingGroups", default)]
    showing_groups: Vec<MultikinoShowingGroup>,
}

#[derive(Debug, Deserialize)]
struct MultikinoShowingGroup {
    date: String,
    #[serde(default)]
    sessions: Vec<MultikinoSession>,
}

#[derive(Debug, Deserialize)]
struct MultikinoSession {
    #[serde(rename = "bookingUrl")]
    booking_url: Option<String>,
    #[serde(rename = "startTime")]
    start_time: String,
    #[serde(rename = "isSoldOut", default)]
    is_sold_out: bool,
    #[serde(rename = "isBookingAvailable", default)]
    is_booking_available: bool,
    #[serde(default)]
    attributes: Vec<MultikinoShowingAttribute>,
}

#[derive(Debug, Deserialize)]
struct MultikinoShowingAttributeGroup {
    name: String,
    #[serde(rename = "showingAttributes", default)]
    showing_attributes: Vec<MultikinoShowingAttribute>,
}

#[derive(Debug, Deserialize)]
struct MultikinoShowingAttribute {
    name: String,
    #[serde(rename = "attributeType")]
    attribute_type: String,
}

#[derive(Default)]
struct AttributeRules {
    format_names: HashSet<String>,
    language_names: HashSet<String>,
}

fn render_play_length(running_time: u16, is_duration_unknown: bool) -> String {
    if is_duration_unknown || running_time == 0 {
        MISSING_DATA_LABEL.to_string()
    } else {
        format!("{running_time} min")
    }
}

fn build_lookup_metadata(base_url: &str, film: &MultikinoFilm) -> MovieLookupMetadata {
    let alternate_titles =
        collect_alternate_titles(&film.film_title, film.original_title.as_deref());
    let premiere_date =
        film.release_date.as_deref().and_then(normalize_api_date).map(str::to_string);

    MovieLookupMetadata {
        chain_movie_id: Some(film.film_id.clone()),
        movie_page_url: film.film_url.as_deref().and_then(|url| build_absolute_url(base_url, url)),
        alternate_titles,
        runtime_minutes: if film.running_time == 0 || film.is_duration_unknown {
            None
        } else {
            Some(film.running_time)
        },
        original_language_code: None,
        genre_tags: normalize_genre_tags(&film.genres),
        production_year: premiere_date.as_deref().and_then(parse_release_year),
        polish_premiere_date: premiere_date,
    }
}

fn collect_alternate_titles(title: &str, original_title: Option<&str>) -> Vec<String> {
    let Some(original_title) = original_title.map(str::trim).filter(|value| !value.is_empty())
    else {
        return Vec::new();
    };
    if normalize_lookup_text(title) == normalize_lookup_text(original_title) {
        Vec::new()
    } else {
        vec![original_title.to_string()]
    }
}

fn normalize_genre_tags(genres: &[String]) -> Vec<String> {
    let mut tags = Vec::new();
    for genre in genres {
        let normalized = normalize_lookup_text(genre);
        if !normalized.is_empty() && !tags.contains(&normalized) {
            tags.push(normalized);
        }
    }
    tags
}

fn join_or_missing(values: &[String]) -> String {
    let joined = values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    if joined.is_empty() { MISSING_DATA_LABEL.to_string() } else { joined }
}

fn select_session_format(
    attributes: &[MultikinoShowingAttribute],
    attribute_rules: &AttributeRules,
) -> String {
    let values = collect_attribute_values(
        attributes,
        "Session",
        &attribute_rules.format_names,
        &IGNORED_SESSION_ATTRIBUTES,
    );
    if values.is_empty() { MISSING_DATA_LABEL.to_string() } else { values.join(" ") }
}

fn select_session_language(
    attributes: &[MultikinoShowingAttribute],
    attribute_rules: &AttributeRules,
) -> String {
    let values =
        collect_attribute_values(attributes, "Language", &attribute_rules.language_names, &[]);
    if values.is_empty() { MISSING_DATA_LABEL.to_string() } else { values.join(" ") }
}

fn collect_attribute_values<'a>(
    attributes: &'a [MultikinoShowingAttribute],
    attribute_type: &str,
    allowed_names: &HashSet<String>,
    ignored_names: &[&str],
) -> Vec<&'a str> {
    let mut values =
        collect_attribute_values_inner(attributes, attribute_type, allowed_names, ignored_names);
    if values.is_empty() && !allowed_names.is_empty() {
        values = collect_attribute_values_inner(
            attributes,
            attribute_type,
            &HashSet::new(),
            ignored_names,
        );
    }
    values
}

fn collect_attribute_values_inner<'a>(
    attributes: &'a [MultikinoShowingAttribute],
    attribute_type: &str,
    allowed_names: &HashSet<String>,
    ignored_names: &[&str],
) -> Vec<&'a str> {
    let mut values = Vec::new();
    for attribute in attributes {
        let name = attribute.name.trim();
        if attribute.attribute_type != attribute_type
            || name.is_empty()
            || ignored_names.contains(&name)
            || (!allowed_names.is_empty() && !allowed_names.contains(name))
            || values.contains(&name)
        {
            continue;
        }
        values.push(name);
    }
    values
}

fn booking_url_for_session(base_url: &str, session: &MultikinoSession) -> Option<String> {
    if session.is_sold_out || !session.is_booking_available {
        return None;
    }
    session.booking_url.as_deref().and_then(|value| build_absolute_url(base_url, value))
}

fn build_absolute_url(base_url: &str, value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        Some(trimmed.to_string())
    } else {
        Some(format!("{}/{}", base_url.trim_end_matches('/'), trimmed.trim_start_matches('/'),))
    }
}

fn extract_showtime_value(start_time: &str) -> Option<String> {
    start_time.split('T').nth(1).and_then(|value| value.get(0..5)).map(str::to_string)
}

fn normalize_api_date(value: &str) -> Option<&str> {
    value.get(0..10)
}

fn parse_release_year(value: &str) -> Option<i32> {
    value.get(0..4)?.parse::<i32>().ok()
}
