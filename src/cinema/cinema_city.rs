use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use async_trait::async_trait;
use chromiumoxide::Page;
use chromiumoxide::browser::{Browser, BrowserConfig};
use futures::StreamExt;
use log::debug;
use regex::Regex;
use reqwest::Client;
use scraper::{ElementRef, Html, Selector};
use serde::Deserialize;
use tokio::time::{Instant, sleep};

use crate::cinema::registry::CinemaChainClient;
use crate::domain::{
    CinemaChainId, CinemaVenue, MovieLookupMetadata, MoviePageFallbackDetails, MoviePlayDetails,
    MoviePlayTime, Repertoire,
};
use crate::error::{AppError, AppResult};
use crate::logging::preview_for_log;
use crate::retry::{RetryDirective, RetryPolicy, retry_with_backoff};

const REQUEST_TIMEOUT_SECONDS: u64 = 30;
const HTML_POLL_INTERVAL_MILLIS: u64 = 250;
const REPERTOIRE_PAGE_READY_SELECTOR: &str = "h2.mr-sm";
const REPERTOIRE_SELECTOR: &str = "div.row.qb-movie";
const CINEMA_VENUES_PAGE_READY_SELECTOR: &str = "body";
const LEGACY_CINEMA_VENUES_SELECTOR: &str = "option[value][data-tokens]";
const MISSING_DATA_LABEL: &str = "Brak danych";
const DEFAULT_CINEMA_CITY_TENANT_ID: &str = "10103";
const DEFAULT_CINEMA_CITY_QUICKBOOK_API_BASE_URL: &str =
    "https://www.cinema-city.pl/pl/data-api-service";
const QUICKBOOK_LANGUAGE: &str = "pl_PL";
const QUICKBOOK_ALTERNATE_TITLE_LANGUAGE: &str = "en_GB";
const CINEMA_CITY_ACCEPT_LANGUAGE: &str = "pl-PL,pl;q=0.9,en-US;q=0.8,en;q=0.7";
const CINEMA_CITY_BROWSER_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
const MAX_LOG_BODY_PREVIEW_CHARS: usize = 256;

static PLAY_LENGTH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d+ min").expect("play length regex must compile"));
static WHITESPACE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+").expect("whitespace regex must compile"));
static TEMPLATE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{([^}]+)\}").expect("template regex must compile"));
static TENANT_ID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"tenantId\s*=\s*"(?P<tenant_id>\d+)""#).expect("tenant id regex must compile")
});

#[async_trait]
pub trait HtmlRenderer: Send + Sync {
    async fn render(&self, url: &str, wait_selector: &str) -> AppResult<String>;
}

#[async_trait]
trait RenderedHtmlSource: Send + Sync {
    async fn content(&self) -> AppResult<String>;
}

#[async_trait]
impl RenderedHtmlSource for Page {
    async fn content(&self) -> AppResult<String> {
        Page::content(self).await.map_err(|error| AppError::BrowserUnavailable(error.to_string()))
    }
}

#[derive(Clone)]
pub struct ChromiumHtmlRenderer;

#[async_trait]
impl HtmlRenderer for ChromiumHtmlRenderer {
    async fn render(&self, url: &str, wait_selector: &str) -> AppResult<String> {
        debug!(
            "Cinema City Chromium render starting url={url} wait_selector={wait_selector} timeout_secs={REQUEST_TIMEOUT_SECONDS}"
        );
        let (mut browser, mut handler) = Browser::launch(
            BrowserConfig::builder()
                .request_timeout(Duration::from_secs(REQUEST_TIMEOUT_SECONDS))
                .build()
                .map_err(|error| AppError::BrowserUnavailable(error.to_string()))?,
        )
        .await
        .map_err(|error| {
            AppError::BrowserUnavailable(format!(
                "Nie udało się uruchomić przeglądarki Chromium: {error}"
            ))
        })?;

        let handler_task = tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                let _ = event;
            }
        });

        let page = browser
            .new_page(url)
            .await
            .map_err(|error| AppError::BrowserUnavailable(error.to_string()))?;
        debug!("Cinema City Chromium page opened url={url} wait_selector={wait_selector}");
        let html = wait_for_selector_in_rendered_html(
            &page,
            url,
            wait_selector,
            Duration::from_secs(REQUEST_TIMEOUT_SECONDS),
            Duration::from_millis(HTML_POLL_INTERVAL_MILLIS),
        )
        .await?;
        debug!("Cinema City Chromium render completed url={url} html_bytes={}", html.len());
        let _ = browser.close().await;
        handler_task.abort();
        Ok(html)
    }
}

#[derive(Clone)]
pub struct CinemaCity {
    repertoire_url: String,
    cinema_venues_url: String,
    quickbook_api_base_url: String,
    http_client: Client,
    renderer: Arc<dyn HtmlRenderer>,
    retry_policy: RetryPolicy,
}

impl CinemaCity {
    pub fn new(
        repertoire_url: String,
        cinema_venues_url: String,
        renderer: Arc<dyn HtmlRenderer>,
    ) -> Self {
        Self {
            repertoire_url,
            cinema_venues_url,
            quickbook_api_base_url: DEFAULT_CINEMA_CITY_QUICKBOOK_API_BASE_URL.to_string(),
            http_client: Client::new(),
            renderer,
            retry_policy: RetryPolicy::network_requests(),
        }
    }

    pub fn with_retry_policy(mut self, retry_policy: RetryPolicy) -> Self {
        self.retry_policy = retry_policy;
        self
    }

    pub fn with_quickbook_api_base_url(
        mut self,
        quickbook_api_base_url: impl Into<String>,
    ) -> Self {
        self.quickbook_api_base_url = quickbook_api_base_url.into();
        self
    }

    fn fill_string_template(text: &str, values: &[(&str, &str)]) -> AppResult<String> {
        let values = values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect::<HashMap<_, _>>();
        let mut rendered = text.to_string();
        for capture in TEMPLATE_RE.captures_iter(text) {
            let variable = capture.get(1).map(|matched| matched.as_str()).unwrap_or_default();
            let replacement = values.get(variable).ok_or_else(|| AppError::TemplateRender {
                missing_variable: variable.to_string(),
            })?;
            rendered = rendered.replace(&format!("{{{variable}}}"), replacement);
        }
        Ok(rendered)
    }

    fn parse_title(movie: &ElementRef<'_>) -> String {
        first_text(movie, "h3.qb-movie-name").unwrap_or_else(|| MISSING_DATA_LABEL.to_string())
    }

    fn parse_genres(movie: &ElementRef<'_>) -> String {
        let raw = first_text(movie, "div.qb-movie-info-wrapper span")
            .unwrap_or_else(|| MISSING_DATA_LABEL.to_string());
        if raw.contains('|') {
            raw.replace('|', "").trim().to_string()
        } else {
            MISSING_DATA_LABEL.to_string()
        }
    }

    fn parse_original_language(movie: &ElementRef<'_>) -> String {
        movie
            .select(selector("span[aria-label]"))
            .find(|element| {
                element
                    .value()
                    .attr("aria-label")
                    .is_some_and(|label| label.contains("original-lang"))
            })
            .map(normalized_text)
            .unwrap_or_else(|| MISSING_DATA_LABEL.to_string())
    }

    fn parse_play_length(movie: &ElementRef<'_>) -> String {
        movie
            .select(selector("div.qb-movie-info-wrapper span"))
            .map(normalized_text)
            .find(|text| PLAY_LENGTH_RE.is_match(text))
            .unwrap_or_else(|| MISSING_DATA_LABEL.to_string())
    }

    fn parse_play_format(play_detail: &ElementRef<'_>) -> String {
        let formats = play_detail
            .select(selector("ul.qb-screening-attributes span[aria-label]"))
            .filter(|element| {
                element
                    .value()
                    .attr("aria-label")
                    .is_some_and(|label| label.contains("Screening type"))
            })
            .map(normalized_text)
            .collect::<Vec<_>>();
        if formats.is_empty() { MISSING_DATA_LABEL.to_string() } else { formats.join(" ") }
    }

    fn parse_play_times(
        play_detail: &ElementRef<'_>,
        movie_page_url: Option<&str>,
    ) -> Vec<MoviePlayTime> {
        play_detail
            .select(selector("a.btn.btn-primary.btn-lg"))
            .map(|play_time| MoviePlayTime {
                value: normalized_text(play_time),
                url: if Self::play_time_has_booking_hint(&play_time) {
                    movie_page_url.map(str::to_string)
                } else {
                    None
                },
            })
            .collect()
    }

    fn parse_play_language(play_detail: &ElementRef<'_>) -> String {
        let prefix = play_detail
            .select(selector("span[aria-label]"))
            .find(|element| {
                element.value().attr("aria-label").is_some_and(|label| {
                    label.contains("subAbbr")
                        || label.contains("dubAbbr")
                        || label.contains("noSubs")
                })
            })
            .map(normalized_text);
        let language = play_detail
            .select(selector("span[aria-label]"))
            .find(|element| {
                element.value().attr("aria-label").is_some_and(|label| {
                    label.contains("subbed-lang")
                        || label.contains("dubbed-lang")
                        || label.contains("first-subbed-lang-")
                        || label.contains("first-dubbed-lang-")
                })
            })
            .map(normalized_text);
        match prefix {
            Some(prefix) if language.as_ref().is_some_and(|value| !value.is_empty()) => {
                format!("{prefix}: {}", language.unwrap_or_default())
            }
            Some(prefix) => prefix,
            None => MISSING_DATA_LABEL.to_string(),
        }
    }

    fn parse_play_details(
        movie: &ElementRef<'_>,
        booking_url: Option<&str>,
    ) -> Vec<MoviePlayDetails> {
        movie
            .select(selector("div.qb-movie-info-column"))
            .map(|play_detail| MoviePlayDetails {
                format: Self::parse_play_format(&play_detail),
                play_language: Self::parse_play_language(&play_detail),
                play_times: Self::parse_play_times(&play_detail, booking_url),
            })
            .collect()
    }

    fn parse_movie_link_url(movie: &ElementRef<'_>) -> Option<String> {
        movie
            .select(selector("a.qb-movie-link[href]"))
            .find_map(|link| link.value().attr("href"))
            .and_then(Self::canonicalize_cinema_city_url)
    }

    fn extract_canonical_movie_page_url(url: &str) -> Option<String> {
        let canonical_url = Self::canonicalize_cinema_city_url(url)?;
        let without_fragment = canonical_url.split('#').next().unwrap_or(&canonical_url);
        let without_query = without_fragment.split('?').next().unwrap_or(without_fragment);
        let normalized = without_query.trim_end_matches('/');
        if normalized.is_empty() { None } else { Some(normalized.to_string()) }
    }

    fn build_lookup_metadata(
        movie_page_url: Option<&str>,
        genres: &str,
        play_length: &str,
        original_language: &str,
    ) -> MovieLookupMetadata {
        let movie_page_url = movie_page_url.map(str::to_string);
        MovieLookupMetadata {
            cinema_city_film_id: movie_page_url
                .as_deref()
                .and_then(Self::extract_movie_id_from_url),
            movie_page_url,
            alternate_titles: Vec::new(),
            runtime_minutes: Self::parse_play_length_minutes(play_length),
            original_language_code: Self::normalize_language_code(original_language),
            genre_tags: Self::normalize_genre_tags(genres),
            production_year: None,
            polish_premiere_date: None,
        }
    }

    fn extract_movie_id_from_url(url: &str) -> Option<String> {
        if let Some(movie_id) = extract_query_param(url, "for-movie") {
            return Some(movie_id);
        }

        url.trim_end_matches('/')
            .rsplit('/')
            .next()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
    }

    fn parse_play_length_minutes(play_length: &str) -> Option<u16> {
        play_length.split_whitespace().next().and_then(|value| value.parse::<u16>().ok())
    }

    fn normalize_language_code(language: &str) -> Option<String> {
        let normalized = Self::normalize_lookup_text(language);
        let code =
            normalized.split_whitespace().find(|value| value.len() == 2 || value.len() == 3)?;
        Some(code.to_ascii_uppercase())
    }

    fn normalize_genre_tags(genres: &str) -> Vec<String> {
        let mut tags = Vec::new();
        for raw_tag in genres.split([',', '|', '/']) {
            let tag = Self::normalize_lookup_text(raw_tag);
            if !tag.is_empty() && !tags.contains(&tag) {
                tags.push(tag);
            }
        }
        tags
    }

    fn normalize_lookup_text(value: &str) -> String {
        let mut normalized = String::new();
        let mut previous_was_separator = false;

        for character in value.chars().map(fold_polish_character_to_ascii) {
            let lowered = character.to_ascii_lowercase();
            if lowered.is_ascii_alphanumeric() {
                normalized.push(lowered);
                previous_was_separator = false;
            } else if !previous_was_separator {
                normalized.push(' ');
                previous_was_separator = true;
            }
        }

        normalized.trim().to_string()
    }

    fn is_non_genre_attribute(attribute_id: &str) -> bool {
        [
            "original lang",
            "original language",
            "subbed lang",
            "dubbed lang",
            "first subbed lang",
            "first dubbed lang",
            "screening type",
            "screen",
            "dub",
            "sub",
            "no subs",
            "no subtitles",
            "2d",
            "3d",
            "4dx",
            "imax",
            "screenx",
            "barco",
            "laser",
            "vip",
            "kids",
            "family",
            "ticket",
            "sales",
            "age",
        ]
        .into_iter()
        .any(|prefix| attribute_id.starts_with(prefix))
    }

    fn extract_original_language_code_from_attribute_ids(
        attribute_ids: &[String],
    ) -> Option<String> {
        attribute_ids.iter().find_map(|attribute_id| {
            let normalized = Self::normalize_lookup_text(attribute_id);
            normalized
                .strip_prefix("original lang ")
                .or_else(|| normalized.strip_prefix("original language "))
                .map(|value| value.to_ascii_uppercase())
        })
    }

    fn extract_genre_tags_from_attribute_ids(attribute_ids: &[String]) -> Vec<String> {
        let mut tags = Vec::new();
        for attribute_id in attribute_ids {
            let normalized = Self::normalize_lookup_text(attribute_id);
            if normalized.is_empty() || Self::is_non_genre_attribute(&normalized) {
                continue;
            }
            let tag = normalized
                .strip_prefix("category ")
                .or_else(|| normalized.strip_prefix("categories "))
                .or_else(|| normalized.strip_prefix("genre "))
                .unwrap_or(&normalized)
                .trim()
                .to_string();
            if !tag.is_empty() && !tags.contains(&tag) {
                tags.push(tag);
            }
        }
        tags
    }

    fn merge_lookup_metadata(current: &mut MovieLookupMetadata, update: &MovieLookupMetadata) {
        if update.cinema_city_film_id.is_some() {
            current.cinema_city_film_id = update.cinema_city_film_id.clone();
        }
        if update.movie_page_url.is_some() {
            current.movie_page_url = update.movie_page_url.clone();
        }
        Self::merge_alternate_titles(&mut current.alternate_titles, &update.alternate_titles);
        if update.runtime_minutes.is_some() {
            current.runtime_minutes = update.runtime_minutes;
        }
        if update.original_language_code.is_some() {
            current.original_language_code = update.original_language_code.clone();
        }
        if !update.genre_tags.is_empty() {
            current.genre_tags = update.genre_tags.clone();
        }
        if update.production_year.is_some() {
            current.production_year = update.production_year;
        }
        if update.polish_premiere_date.is_some() {
            current.polish_premiere_date = update.polish_premiere_date.clone();
        }
    }

    fn merge_alternate_titles(current: &mut Vec<String>, update: &[String]) {
        for title in update {
            let normalized_update = Self::normalize_lookup_text(title);
            if normalized_update.is_empty()
                || current
                    .iter()
                    .any(|existing| Self::normalize_lookup_text(existing) == normalized_update)
            {
                continue;
            }
            current.push(title.clone());
        }
    }

    fn play_time_has_booking_hint(play_time: &ElementRef<'_>) -> bool {
        ["href", "ng-href", "data-ng-href", "ng-click", "data-ng-click", "onclick"].into_iter().any(
            |attribute_name| {
                play_time.value().attr(attribute_name).is_some_and(|value| !value.trim().is_empty())
            },
        )
    }

    fn is_presale(movie: &ElementRef<'_>) -> bool {
        movie
            .select(selector("div.qb-movie-info-column h4"))
            .any(|element| normalized_text(element).to_uppercase().contains("PRZEDSPRZED"))
    }

    fn parse_legacy_venues(html: &Html) -> Vec<CinemaVenue> {
        html.select(selector(LEGACY_CINEMA_VENUES_SELECTOR))
            .filter_map(|cinema| {
                let venue_name = cinema.value().attr("data-tokens")?.trim().to_string();
                let venue_id = cinema.value().attr("value")?.trim().to_string();
                if venue_name.is_empty()
                    || venue_name == "null"
                    || !venue_id.chars().all(|character| character.is_ascii_digit())
                {
                    return None;
                }
                Some(CinemaVenue {
                    chain_id: CinemaChainId::CinemaCity.as_str().to_string(),
                    venue_name,
                    venue_id,
                })
            })
            .collect()
    }

    fn parse_api_sites_list_venues(rendered_html: &str) -> AppResult<Vec<CinemaVenue>> {
        let Some(api_sites_list) = extract_json_array_assignment(rendered_html, "apiSitesList")
        else {
            debug!(
                "Cinema City venues page did not include an apiSitesList assignment; html_preview={}",
                preview_for_log(rendered_html, MAX_LOG_BODY_PREVIEW_CHARS),
            );
            return Ok(Vec::new());
        };

        let parsed_sites = serde_json::from_str::<Vec<CinemaCityApiSite>>(api_sites_list)
            .map_err(|error| {
                debug!(
                    "Cinema City venues JSON parse failed error={error} payload_preview={}",
                    preview_for_log(api_sites_list, MAX_LOG_BODY_PREVIEW_CHARS),
                );
                AppError::BrowserUnavailable(format!(
                    "Nie udało się odczytać listy lokali Cinema City z aktualnego formatu strony: {error}"
                ))
            })?;

        Ok(parsed_sites.into_iter().filter_map(CinemaCityApiSite::into_venue).collect())
    }

    fn normalize_venue_label(label: &str) -> String {
        let folded = label.chars().map(fold_polish_character_to_ascii).collect::<String>();
        WHITESPACE_RE.replace_all(folded.trim(), " ").to_string()
    }

    fn build_venue_slug(venue_name: &str) -> String {
        let slug_source = venue_name.rsplit(" - ").next().unwrap_or(venue_name);
        let mut slug = String::new();
        let mut previous_was_separator = false;

        for character in slug_source.chars().map(fold_polish_character_to_ascii) {
            let lowered = character.to_ascii_lowercase();
            if lowered.is_ascii_alphanumeric() {
                slug.push(lowered);
                previous_was_separator = false;
            } else if !previous_was_separator {
                slug.push('-');
                previous_was_separator = true;
            }
        }

        slug.trim_matches('-').to_string()
    }

    fn canonical_repertoire_url(venue: &CinemaVenue, date: &str) -> String {
        let venue_slug = Self::build_venue_slug(&venue.venue_name);
        format!(
            "https://www.cinema-city.pl/kina/{venue_slug}/{venue_id}#/buy-tickets-by-cinema?in-cinema={venue_id}&at={date}&view-mode=list",
            venue_id = venue.venue_id,
        )
    }

    fn uses_legacy_repertoire_template(template: &str) -> bool {
        template.contains("/#/buy-tickets-by-cinema") && !template.contains("{cinema_venue_slug}")
    }

    fn build_repertoire_url(&self, venue: &CinemaVenue, date: &str) -> AppResult<String> {
        if Self::uses_legacy_repertoire_template(&self.repertoire_url) {
            return Ok(Self::canonical_repertoire_url(venue, date));
        }

        let venue_slug = Self::build_venue_slug(&venue.venue_name);
        Self::fill_string_template(
            &self.repertoire_url,
            &[
                ("cinema_venue_id", venue.venue_id.as_str()),
                ("cinema_venue_slug", venue_slug.as_str()),
                ("repertoire_date", date),
            ],
        )
    }

    fn build_quickbook_film_events_url(
        &self,
        tenant_id: &str,
        venue: &CinemaVenue,
        date: &str,
        language: &str,
    ) -> String {
        format!(
            "{}/v1/quickbook/{tenant_id}/film-events/in-cinema/{venue_id}/at-date/{date}?attr=&lang={language}",
            self.quickbook_api_base_url.trim_end_matches('/'),
            venue_id = venue.venue_id,
        )
    }

    fn extract_tenant_id(rendered_html: &str) -> Option<String> {
        TENANT_ID_RE
            .captures(rendered_html)
            .and_then(|captures| captures.name("tenant_id"))
            .map(|tenant_id| tenant_id.as_str().to_string())
    }

    fn extract_showtime_value(event_date_time: &str) -> Option<String> {
        event_date_time.get(11..16).map(str::to_string)
    }

    fn canonicalize_cinema_city_url(url: &str) -> Option<String> {
        let trimmed = url.trim();
        if trimmed.is_empty() {
            return None;
        }

        if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
            return Some(trimmed.to_string());
        }

        if trimmed.starts_with("//") {
            return Some(format!("https:{trimmed}"));
        }

        if trimmed.starts_with('/') {
            return Some(format!("https://www.cinema-city.pl{trimmed}"));
        }

        None
    }

    async fn fetch_quickbook_movie_enrichment(
        &self,
        rendered_html: &str,
        date: &str,
        venue: &CinemaVenue,
    ) -> AppResult<HashMap<String, QuickbookMovieEnrichment>> {
        if self.quickbook_api_base_url.trim().is_empty() {
            debug!(
                "Cinema City quickbook enrichment skipped because no API base URL is configured venue_id={} date={date}",
                venue.venue_id,
            );
            return Ok(HashMap::new());
        }

        let tenant_id = Self::extract_tenant_id(rendered_html)
            .unwrap_or_else(|| {
                debug!(
                    "Cinema City tenant id was missing from rendered HTML; falling back to default tenant_id={DEFAULT_CINEMA_CITY_TENANT_ID}"
                );
                DEFAULT_CINEMA_CITY_TENANT_ID.to_string()
            });
        let repertoire_url = self.build_repertoire_url(venue, date)?;
        let payload = self
            .fetch_quickbook_film_events_payload(
                &tenant_id,
                venue,
                date,
                QUICKBOOK_LANGUAGE,
                &repertoire_url,
            )
            .await?;
        let alternate_titles_by_id = match self
            .fetch_quickbook_alternate_titles(
                &tenant_id,
                venue,
                date,
                &repertoire_url,
                &payload.body.films,
            )
            .await
        {
            Ok(alternate_titles) => alternate_titles,
            Err(error) => {
                debug!(
                    "Cinema City quickbook alternate-title enrichment skipped venue_id={} date={date} error={error}",
                    venue.venue_id,
                );
                HashMap::new()
            }
        };

        let films_count = payload.body.films.len();
        let events_count = payload.body.events.len();
        debug!(
            "Cinema City quickbook payload parsed venue_id={} date={date} films={films_count} events={events_count} alternate_title_entries={}",
            venue.venue_id,
            alternate_titles_by_id.len(),
        );

        let films_by_id = payload
            .body
            .films
            .into_iter()
            .map(|film| {
                let movie_page_url =
                    film.link.as_deref().and_then(Self::canonicalize_cinema_city_url);
                let production_year =
                    film.release_year.as_deref().and_then(|value| value.parse::<i32>().ok());
                let polish_premiere_date =
                    film.release_date.filter(|value| !value.trim().is_empty());
                let original_language_code =
                    Self::extract_original_language_code_from_attribute_ids(&film.attribute_ids);
                let genre_tags = Self::extract_genre_tags_from_attribute_ids(&film.attribute_ids);
                let film_id = film.id;
                let alternate_titles = alternate_titles_by_id
                    .get(&film_id)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|title| {
                        Self::normalize_lookup_text(title)
                            != Self::normalize_lookup_text(&film.name)
                    })
                    .collect::<Vec<_>>();
                (
                    film_id.clone(),
                    BookableMovieMetadata {
                        title: film.name,
                        lookup_metadata: MovieLookupMetadata {
                            cinema_city_film_id: Some(film_id),
                            movie_page_url,
                            alternate_titles,
                            runtime_minutes: film.length,
                            original_language_code,
                            genre_tags,
                            production_year,
                            polish_premiere_date,
                        },
                    },
                )
            })
            .collect::<HashMap<_, _>>();

        let mut quickbook_movies = HashMap::<String, QuickbookMovieEnrichment>::new();
        for movie in films_by_id.values() {
            let entry =
                quickbook_movies.entry(Self::normalize_lookup_text(&movie.title)).or_default();
            Self::merge_lookup_metadata(&mut entry.lookup_metadata, &movie.lookup_metadata);
        }

        for event in payload.body.events {
            if event.sold_out
                || event.booking_link.as_deref().is_none_or(|value| value.trim().is_empty())
            {
                continue;
            }
            if event
                .composite_booking_link
                .as_ref()
                .is_some_and(|composite_booking_link| composite_booking_link.block_online_sales)
            {
                continue;
            }

            let Some(movie) = films_by_id.get(&event.film_id) else {
                debug!(
                    "Cinema City quickbook event referenced an unknown film id={} venue_id={} date={date}",
                    event.film_id, venue.venue_id,
                );
                continue;
            };
            let Some(showtime_value) = Self::extract_showtime_value(&event.event_date_time) else {
                debug!(
                    "Cinema City quickbook event had an invalid eventDateTime value film_id={} event_date_time={} venue_id={} date={date}",
                    event.film_id, event.event_date_time, venue.venue_id,
                );
                continue;
            };

            let entry =
                quickbook_movies.entry(Self::normalize_lookup_text(&movie.title)).or_default();
            Self::merge_lookup_metadata(&mut entry.lookup_metadata, &movie.lookup_metadata);
            entry.showtimes.insert(showtime_value);
        }

        debug!(
            "Cinema City quickbook enrichment built entries={} venue_id={} date={date}",
            quickbook_movies.len(),
            venue.venue_id,
        );
        Ok(quickbook_movies)
    }

    async fn fetch_quickbook_film_events_payload(
        &self,
        tenant_id: &str,
        venue: &CinemaVenue,
        date: &str,
        language: &str,
        repertoire_url: &str,
    ) -> AppResult<CinemaCityFilmEventsResponse> {
        let quickbook_url = self.build_quickbook_film_events_url(tenant_id, venue, date, language);
        debug!(
            "Cinema City quickbook request url={quickbook_url} venue_id={} date={date} tenant_id={tenant_id} language={language}",
            venue.venue_id
        );
        let response = self
            .http_client
            .get(&quickbook_url)
            .header(reqwest::header::USER_AGENT, CINEMA_CITY_BROWSER_USER_AGENT)
            .header(reqwest::header::ACCEPT, "application/json, text/plain, */*")
            .header(reqwest::header::ACCEPT_LANGUAGE, CINEMA_CITY_ACCEPT_LANGUAGE)
            .header(reqwest::header::REFERER, repertoire_url)
            .header("X-Requested-With", "XMLHttpRequest")
            .send()
            .await
            .map_err(|error| {
                debug!(
                    "Cinema City quickbook request transport failed url={quickbook_url} venue_id={} date={date} language={language} error={error}",
                    venue.venue_id
                );
                AppError::Http(format!(
                    "Nie udało się pobrać danych o rezerwacjach z API Cinema City: {error}"
                ))
            })?;
        let status = response.status();
        debug!(
            "Cinema City quickbook response url={quickbook_url} venue_id={} date={date} language={language} status={status}",
            venue.venue_id
        );
        if status.is_client_error() || status.is_server_error() {
            let body_preview = response_body_preview(response).await;
            debug!(
                "Cinema City quickbook request failed url={quickbook_url} venue_id={} date={date} language={language} status={status} body_preview={body_preview}",
                venue.venue_id
            );
            return Err(AppError::Http(format!(
                "API Cinema City zwróciło błąd podczas pobierania danych o rezerwacjach: status {status}"
            )));
        }
        let body = response.text().await.map_err(|error| {
            debug!(
                "Cinema City quickbook response read failed url={quickbook_url} venue_id={} date={date} language={language} error={error}",
                venue.venue_id
            );
            AppError::Http(format!(
                "Nie udało się odczytać danych o rezerwacjach z API Cinema City: {error}"
            ))
        })?;
        debug!(
            "Cinema City quickbook response body received url={quickbook_url} venue_id={} date={date} language={language} bytes={}",
            venue.venue_id,
            body.len(),
        );
        serde_json::from_str::<CinemaCityFilmEventsResponse>(&body).map_err(|error| {
            debug!(
                "Cinema City quickbook JSON parse failed url={quickbook_url} venue_id={} date={date} language={language} error={error} body_preview={}",
                venue.venue_id,
                preview_for_log(&body, MAX_LOG_BODY_PREVIEW_CHARS),
            );
            AppError::Http(format!(
                "Nie udało się odczytać danych o rezerwacjach z API Cinema City: {error}"
            ))
        })
    }

    async fn fetch_quickbook_alternate_titles(
        &self,
        tenant_id: &str,
        venue: &CinemaVenue,
        date: &str,
        repertoire_url: &str,
        base_films: &[CinemaCityFilmEventFilm],
    ) -> AppResult<HashMap<String, Vec<String>>> {
        let payload = self
            .fetch_quickbook_film_events_payload(
                tenant_id,
                venue,
                date,
                QUICKBOOK_ALTERNATE_TITLE_LANGUAGE,
                repertoire_url,
            )
            .await?;
        let base_titles_by_id = base_films
            .iter()
            .map(|film| (film.id.as_str(), Self::normalize_lookup_text(&film.name)))
            .collect::<HashMap<_, _>>();
        let mut alternate_titles_by_id = HashMap::<String, Vec<String>>::new();
        for film in payload.body.films {
            let normalized_title = Self::normalize_lookup_text(&film.name);
            if normalized_title.is_empty() {
                continue;
            }
            if base_titles_by_id
                .get(film.id.as_str())
                .is_some_and(|base_title| base_title == &normalized_title)
            {
                continue;
            }
            let entry = alternate_titles_by_id.entry(film.id).or_default();
            if entry.iter().any(|title| Self::normalize_lookup_text(title) == normalized_title) {
                continue;
            }
            entry.push(film.name);
        }
        Ok(alternate_titles_by_id)
    }

    fn apply_quickbook_movie_enrichment(
        repertoire: &mut [Repertoire],
        quickbook_movies: &HashMap<String, QuickbookMovieEnrichment>,
    ) {
        for movie in repertoire {
            let Some(quickbook_movie) =
                quickbook_movies.get(&Self::normalize_lookup_text(&movie.title))
            else {
                continue;
            };

            Self::merge_lookup_metadata(
                &mut movie.lookup_metadata,
                &quickbook_movie.lookup_metadata,
            );
            let Some(movie_page_url) = movie.lookup_metadata.movie_page_url.clone() else {
                continue;
            };

            for play_detail in &mut movie.play_details {
                for play_time in &mut play_detail.play_times {
                    play_time.url = if quickbook_movie.showtimes.contains(&play_time.value) {
                        play_time.url.clone().or_else(|| Some(movie_page_url.clone()))
                    } else {
                        None
                    };
                }
            }
        }
    }

    async fn render_with_retry(&self, url: &str, wait_selector: &str) -> AppResult<String> {
        retry_with_backoff(self.retry_policy, |attempt| async move {
            debug!("Cinema City render attempt={attempt} url={url} wait_selector={wait_selector}");
            self.renderer
                .render(url, wait_selector)
                .await
                .map_err(|error| classify_render_error(attempt, url, wait_selector, error))
        })
        .await
    }
}

#[async_trait]
impl CinemaChainClient for CinemaCity {
    async fn fetch_repertoire(
        &self,
        date: &str,
        venue: &CinemaVenue,
    ) -> AppResult<Vec<Repertoire>> {
        let url = self.build_repertoire_url(venue, date)?;
        debug!(
            "Cinema City repertoire fetch starting url={url} venue_id={} venue_name={:?} date={date}",
            venue.venue_id, venue.venue_name,
        );
        let rendered_html = self.render_with_retry(&url, REPERTOIRE_PAGE_READY_SELECTOR).await?;
        let mut repertoire = {
            let html = Html::parse_document(&rendered_html);
            html.select(selector(REPERTOIRE_SELECTOR))
                .filter(|movie| !Self::is_presale(movie))
                .map(|movie| {
                    let title = Self::parse_title(&movie);
                    let genres = Self::parse_genres(&movie);
                    let play_length = Self::parse_play_length(&movie);
                    let original_language = Self::parse_original_language(&movie);
                    let booking_url = Self::parse_movie_link_url(&movie);
                    let movie_page_url =
                        booking_url.as_deref().and_then(Self::extract_canonical_movie_page_url);
                    Repertoire {
                        title,
                        genres: genres.clone(),
                        play_length: play_length.clone(),
                        original_language: original_language.clone(),
                        play_details: Self::parse_play_details(&movie, booking_url.as_deref()),
                        lookup_metadata: Self::build_lookup_metadata(
                            movie_page_url.as_deref(),
                            &genres,
                            &play_length,
                            &original_language,
                        ),
                    }
                })
                .collect::<Vec<_>>()
        };
        debug!(
            "Cinema City repertoire parsed url={url} venue_id={} date={date} movies={} html_bytes={}",
            venue.venue_id,
            repertoire.len(),
            rendered_html.len(),
        );

        match self.fetch_quickbook_movie_enrichment(&rendered_html, date, venue).await {
            Ok(quickbook_movies) => {
                debug!(
                    "Cinema City repertoire enrichment applying quickbook entries={} venue_id={} date={date}",
                    quickbook_movies.len(),
                    venue.venue_id,
                );
                Self::apply_quickbook_movie_enrichment(&mut repertoire, &quickbook_movies);
            }
            Err(error) => {
                debug!(
                    "Cinema City repertoire enrichment skipped because quickbook lookup failed venue_id={} date={date} error={error}",
                    venue.venue_id,
                );
            }
        }

        Ok(repertoire)
    }

    async fn fetch_venues(&self) -> AppResult<Vec<CinemaVenue>> {
        debug!("Cinema City venues fetch starting url={}", self.cinema_venues_url);
        let rendered_html = self
            .render_with_retry(&self.cinema_venues_url, CINEMA_VENUES_PAGE_READY_SELECTOR)
            .await?;
        let html = Html::parse_document(&rendered_html);
        let legacy_venues = Self::parse_legacy_venues(&html);
        if !legacy_venues.is_empty() {
            debug!(
                "Cinema City venues parsed legacy markup count={} html_bytes={}",
                legacy_venues.len(),
                rendered_html.len(),
            );
            return Ok(legacy_venues);
        }

        let venues = Self::parse_api_sites_list_venues(&rendered_html)?;
        debug!(
            "Cinema City venues parsed embedded apiSitesList count={} html_bytes={}",
            venues.len(),
            rendered_html.len(),
        );
        Ok(venues)
    }
}

#[derive(Debug, Deserialize)]
struct CinemaCityApiSite {
    #[serde(rename = "externalCode")]
    external_code: String,
    name: String,
    address: Option<CinemaCityApiSiteAddress>,
}

impl CinemaCityApiSite {
    fn into_venue(self) -> Option<CinemaVenue> {
        let venue_id = self.external_code.trim();
        if venue_id.is_empty() || !venue_id.chars().all(|character| character.is_ascii_digit()) {
            return None;
        }

        let normalized_name = CinemaCity::normalize_venue_label(&self.name);
        if normalized_name.is_empty() {
            return None;
        }

        let venue_name = match self
            .address
            .and_then(|address| address.city)
            .map(|city| CinemaCity::normalize_venue_label(&city))
        {
            Some(city) if !city.is_empty() && city != normalized_name => {
                let city_with_separator = format!("{city} - ");
                if normalized_name.starts_with(&city_with_separator) {
                    normalized_name
                } else {
                    match normalized_name.strip_prefix(&format!("{city} ")) {
                        Some(rest) if !rest.is_empty() => format!("{city} - {rest}"),
                        _ => normalized_name,
                    }
                }
            }
            _ => normalized_name,
        };

        Some(CinemaVenue {
            chain_id: CinemaChainId::CinemaCity.as_str().to_string(),
            venue_name,
            venue_id: venue_id.to_string(),
        })
    }
}

#[derive(Debug, Deserialize)]
struct CinemaCityApiSiteAddress {
    city: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CinemaCityFilmEventsResponse {
    body: CinemaCityFilmEventsBody,
}

#[derive(Debug, Deserialize)]
struct CinemaCityFilmEventsBody {
    films: Vec<CinemaCityFilmEventFilm>,
    events: Vec<CinemaCityFilmEvent>,
}

#[derive(Debug, Deserialize)]
struct CinemaCityFilmEventFilm {
    id: String,
    name: String,
    link: Option<String>,
    length: Option<u16>,
    #[serde(rename = "releaseYear")]
    release_year: Option<String>,
    #[serde(rename = "releaseDate")]
    release_date: Option<String>,
    #[serde(rename = "attributeIds", default)]
    attribute_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CinemaCityFilmEvent {
    #[serde(rename = "filmId")]
    film_id: String,
    #[serde(rename = "eventDateTime")]
    event_date_time: String,
    #[serde(rename = "bookingLink")]
    booking_link: Option<String>,
    #[serde(rename = "soldOut")]
    sold_out: bool,
    #[serde(rename = "compositeBookingLink")]
    composite_booking_link: Option<CinemaCityCompositeBookingLink>,
}

#[derive(Debug, Deserialize)]
struct CinemaCityCompositeBookingLink {
    #[serde(rename = "blockOnlineSales")]
    block_online_sales: bool,
}

#[derive(Debug, Deserialize)]
struct CinemaCityMoviePageDetails {
    #[serde(rename = "originalName")]
    original_name: Option<String>,
    #[serde(rename = "releaseCountry")]
    release_country: Option<String>,
    cast: Option<String>,
    directors: Option<String>,
    synopsis: Option<String>,
}

#[derive(Debug, Clone)]
struct BookableMovieMetadata {
    title: String,
    lookup_metadata: MovieLookupMetadata,
}

#[derive(Debug, Clone, Default)]
struct QuickbookMovieEnrichment {
    lookup_metadata: MovieLookupMetadata,
    showtimes: HashSet<String>,
}

pub fn parse_movie_page_fallback_details(
    rendered_html: &str,
) -> AppResult<MoviePageFallbackDetails> {
    let Some(film_details_json) = extract_json_object_assignment(rendered_html, "filmDetails")
    else {
        debug!(
            "Cinema City movie page did not include a filmDetails assignment; html_preview={}",
            preview_for_log(rendered_html, MAX_LOG_BODY_PREVIEW_CHARS),
        );
        return Err(AppError::BrowserUnavailable(
            "Nie udało się odczytać szczegółów filmu Cinema City z aktualnego formatu strony."
                .to_string(),
        ));
    };

    let details = serde_json::from_str::<CinemaCityMoviePageDetails>(film_details_json).map_err(
        |error| {
            debug!(
                "Cinema City movie page filmDetails JSON parse failed error={error} payload_preview={}",
                preview_for_log(film_details_json, MAX_LOG_BODY_PREVIEW_CHARS),
            );
            AppError::BrowserUnavailable(format!(
                "Nie udało się odczytać szczegółów filmu Cinema City z aktualnego formatu strony: {error}"
            ))
        },
    )?;

    Ok(MoviePageFallbackDetails {
        original_title: normalize_optional_text(details.original_name),
        country: normalize_optional_text(details.release_country),
        cast: split_people_list(details.cast.as_deref()),
        directors: split_people_list(details.directors.as_deref()),
        synopsis: normalize_optional_text(details.synopsis),
    })
}

fn selector(value: &str) -> &'static Selector {
    Box::leak(Box::new(Selector::parse(value).expect("selector must compile")))
}

fn extract_json_array_assignment<'a>(html: &'a str, variable_name: &str) -> Option<&'a str> {
    extract_json_assignment(html, variable_name, '[', ']')
}

fn extract_json_object_assignment<'a>(html: &'a str, variable_name: &str) -> Option<&'a str> {
    extract_json_assignment(html, variable_name, '{', '}')
}

fn extract_json_assignment<'a>(
    html: &'a str,
    variable_name: &str,
    open_char: char,
    close_char: char,
) -> Option<&'a str> {
    let start = html.find(&format!("{variable_name} = {open_char}"))?;
    let json_start = start + html[start..].find(open_char)?;
    let mut depth = 0;
    let mut inside_string = false;
    let mut escaped = false;

    for (offset, character) in html[json_start..].char_indices() {
        if inside_string {
            match character {
                '\\' if !escaped => escaped = true,
                '"' if !escaped => inside_string = false,
                _ => escaped = false,
            }
            continue;
        }

        match character {
            '"' => inside_string = true,
            character if character == open_char => depth += 1,
            character if character == close_char => {
                depth -= 1;
                if depth == 0 {
                    let json_end = json_start + offset + character.len_utf8();
                    return Some(&html[json_start..json_end]);
                }
            }
            _ => {}
        }
    }

    None
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.map(|value| value.trim().to_string()).filter(|value| !value.is_empty())
}

fn split_people_list(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split([',', ';'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn extract_query_param(url: &str, parameter_name: &str) -> Option<String> {
    url.split(['?', '#', '&'])
        .filter_map(|segment| segment.split_once('='))
        .find(|(name, _)| *name == parameter_name)
        .map(|(_, value)| value.to_string())
        .filter(|value| !value.trim().is_empty())
}

fn first_text(element: &ElementRef<'_>, selector_value: &str) -> Option<String> {
    element.select(selector(selector_value)).next().map(normalized_text)
}

fn normalized_text(element: ElementRef<'_>) -> String {
    WHITESPACE_RE.replace_all(&element.text().collect::<String>(), " ").trim().to_string()
}

fn fold_polish_character_to_ascii(character: char) -> char {
    match character {
        'ą' => 'a',
        'ć' => 'c',
        'ę' => 'e',
        'ł' => 'l',
        'ń' => 'n',
        'ó' => 'o',
        'ś' => 's',
        'ź' | 'ż' => 'z',
        'Ą' => 'A',
        'Ć' => 'C',
        'Ę' => 'E',
        'Ł' => 'L',
        'Ń' => 'N',
        'Ó' => 'O',
        'Ś' => 'S',
        'Ź' | 'Ż' => 'Z',
        _ => character,
    }
}

async fn wait_for_selector_in_rendered_html(
    source: &dyn RenderedHtmlSource,
    url: &str,
    wait_selector: &str,
    timeout: Duration,
    poll_interval: Duration,
) -> AppResult<String> {
    let selector = Selector::parse(wait_selector).expect("wait selector must compile");
    let deadline = Instant::now() + timeout;
    let mut last_transient_error = None;
    let mut is_first_attempt = true;
    let mut poll_attempt = 0_usize;

    loop {
        poll_attempt += 1;
        if !is_first_attempt && Instant::now() >= deadline {
            debug!(
                "Cinema City rendered HTML wait timed out url={url} wait_selector={wait_selector} attempts={poll_attempt} last_transient_error={last_transient_error:?}"
            );
            return Err(build_wait_timeout_error(
                url,
                wait_selector,
                last_transient_error.as_ref(),
            ));
        }
        is_first_attempt = false;

        match source.content().await {
            Ok(rendered_html) => {
                if Html::parse_document(&rendered_html).select(&selector).next().is_some() {
                    debug!(
                        "Cinema City rendered HTML selector found url={url} wait_selector={wait_selector} attempts={poll_attempt} html_bytes={}",
                        rendered_html.len(),
                    );
                    return Ok(rendered_html);
                }
            }
            Err(error) if is_transient_browser_error(&error) => {
                debug!(
                    "Cinema City rendered HTML source returned a transient browser error url={url} wait_selector={wait_selector} attempts={poll_attempt} error={error}"
                );
                last_transient_error = Some(error);
            }
            Err(error) => return Err(error),
        }

        if Instant::now() >= deadline {
            debug!(
                "Cinema City rendered HTML wait timed out url={url} wait_selector={wait_selector} attempts={poll_attempt} last_transient_error={last_transient_error:?}"
            );
            return Err(build_wait_timeout_error(
                url,
                wait_selector,
                last_transient_error.as_ref(),
            ));
        }

        sleep(poll_interval).await;
    }
}

fn is_transient_browser_error(error: &AppError) -> bool {
    match error {
        AppError::BrowserUnavailable(message) => {
            message.contains("Could not find node with given id")
                || message.contains("Cannot find context with specified id")
                || message.contains("Execution context was destroyed")
        }
        _ => false,
    }
}

fn build_wait_timeout_error(
    url: &str,
    wait_selector: &str,
    last_transient_error: Option<&AppError>,
) -> AppError {
    let base_message = format!(
        "Przekroczono limit czasu podczas oczekiwania na element `{wait_selector}` na stronie {url}."
    );
    match last_transient_error {
        Some(error) => AppError::BrowserUnavailable(format!(
            "{base_message} Ostatni błąd przeglądarki: {error}"
        )),
        None => AppError::BrowserUnavailable(base_message),
    }
}

fn classify_render_error(
    attempt: usize,
    url: &str,
    wait_selector: &str,
    error: AppError,
) -> RetryDirective<AppError> {
    match error {
        error @ AppError::BrowserUnavailable(_) => {
            debug!(
                "Cinema City render failed with a retryable browser error attempt={attempt} url={url} wait_selector={wait_selector} error={error}"
            );
            RetryDirective::retry(error)
        }
        error => {
            debug!(
                "Cinema City render failed with a non-retryable error attempt={attempt} url={url} wait_selector={wait_selector} error={error}"
            );
            RetryDirective::fail(error)
        }
    }
}

async fn response_body_preview(response: reqwest::Response) -> String {
    match response.text().await {
        Ok(body) if body.trim().is_empty() => "<empty>".to_string(),
        Ok(body) => preview_for_log(&body, MAX_LOG_BODY_PREVIEW_CHARS),
        Err(error) => format!("<unavailable: {error}>"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use super::*;

    struct FakeRenderedHtmlSource {
        responses: Mutex<VecDeque<AppResult<String>>>,
    }

    impl FakeRenderedHtmlSource {
        fn new(responses: Vec<AppResult<String>>) -> Self {
            Self { responses: Mutex::new(VecDeque::from(responses)) }
        }
    }

    #[async_trait]
    impl RenderedHtmlSource for FakeRenderedHtmlSource {
        async fn content(&self) -> AppResult<String> {
            self.responses
                .lock()
                .expect("rendered html responses lock poisoned")
                .pop_front()
                .expect("test response must be configured")
        }
    }

    #[tokio::test]
    async fn wait_for_selector_in_rendered_html_retries_transient_missing_node_errors() {
        let source = FakeRenderedHtmlSource::new(vec![
            Err(AppError::BrowserUnavailable(
                "Error -32000: Could not find node with given id".to_string(),
            )),
            Ok("<html><body><div>loading...</div></body></html>".to_string()),
            Ok("<html><body><div class=\"row qb-movie\">loaded</div></body></html>".to_string()),
        ]);

        let rendered_html = wait_for_selector_in_rendered_html(
            &source,
            "https://example.test/repertoire",
            REPERTOIRE_SELECTOR,
            Duration::from_millis(50),
            Duration::from_millis(1),
        )
        .await
        .expect("transient node lookup errors should be retried");

        assert!(rendered_html.contains("qb-movie"));
    }

    #[tokio::test]
    async fn wait_for_selector_in_rendered_html_times_out_when_selector_never_appears() {
        let source = FakeRenderedHtmlSource::new(vec![
            Ok("<html><body><div>loading...</div></body></html>".to_string()),
            Ok("<html><body><div>still loading...</div></body></html>".to_string()),
        ]);

        let error = wait_for_selector_in_rendered_html(
            &source,
            "https://example.test/repertoire",
            REPERTOIRE_SELECTOR,
            Duration::from_millis(2),
            Duration::from_millis(1),
        )
        .await
        .expect_err("missing selector should time out");

        assert_eq!(
            error,
            AppError::BrowserUnavailable(
                "Przekroczono limit czasu podczas oczekiwania na element `div.row.qb-movie` na stronie https://example.test/repertoire."
                    .to_string(),
            )
        );
    }
}
