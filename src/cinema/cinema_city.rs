use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use async_trait::async_trait;
use chromiumoxide::Page;
use chromiumoxide::browser::{Browser, BrowserConfig};
use futures::StreamExt;
use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use serde::Deserialize;
use tokio::time::{Instant, sleep};

use crate::cinema::registry::CinemaChainClient;
use crate::domain::{CinemaChainId, CinemaVenue, MoviePlayDetails, Repertoire};
use crate::error::{AppError, AppResult};
use crate::retry::{RetryDirective, RetryPolicy, retry_with_backoff};

const REQUEST_TIMEOUT_SECONDS: u64 = 30;
const HTML_POLL_INTERVAL_MILLIS: u64 = 250;
const REPERTOIRE_PAGE_READY_SELECTOR: &str = "h2.mr-sm";
const REPERTOIRE_SELECTOR: &str = "div.row.qb-movie";
const CINEMA_VENUES_PAGE_READY_SELECTOR: &str = "body";
const LEGACY_CINEMA_VENUES_SELECTOR: &str = "option[value][data-tokens]";
const MISSING_DATA_LABEL: &str = "Brak danych";

static PLAY_LENGTH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d+ min").expect("play length regex must compile"));
static WHITESPACE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+").expect("whitespace regex must compile"));
static TEMPLATE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{([^}]+)\}").expect("template regex must compile"));

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
        let html = wait_for_selector_in_rendered_html(
            &page,
            url,
            wait_selector,
            Duration::from_secs(REQUEST_TIMEOUT_SECONDS),
            Duration::from_millis(HTML_POLL_INTERVAL_MILLIS),
        )
        .await?;
        let _ = browser.close().await;
        handler_task.abort();
        Ok(html)
    }
}

#[derive(Clone)]
pub struct CinemaCity {
    repertoire_url: String,
    cinema_venues_url: String,
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
            renderer,
            retry_policy: RetryPolicy::network_requests(),
        }
    }

    pub fn with_retry_policy(mut self, retry_policy: RetryPolicy) -> Self {
        self.retry_policy = retry_policy;
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

    fn parse_play_times(play_detail: &ElementRef<'_>) -> Vec<String> {
        play_detail.select(selector("a.btn.btn-primary.btn-lg")).map(normalized_text).collect()
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

    fn parse_play_details(movie: &ElementRef<'_>) -> Vec<MoviePlayDetails> {
        movie
            .select(selector("div.qb-movie-info-column"))
            .map(|play_detail| MoviePlayDetails {
                format: Self::parse_play_format(&play_detail),
                play_language: Self::parse_play_language(&play_detail),
                play_times: Self::parse_play_times(&play_detail),
            })
            .collect()
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
            return Ok(Vec::new());
        };

        let parsed_sites = serde_json::from_str::<Vec<CinemaCityApiSite>>(api_sites_list)
            .map_err(|error| {
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

    async fn render_with_retry(&self, url: &str, wait_selector: &str) -> AppResult<String> {
        retry_with_backoff(self.retry_policy, |_| async {
            self.renderer.render(url, wait_selector).await.map_err(classify_render_error)
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
        let rendered_html = self.render_with_retry(&url, REPERTOIRE_PAGE_READY_SELECTOR).await?;
        let html = Html::parse_document(&rendered_html);
        Ok(html
            .select(selector(REPERTOIRE_SELECTOR))
            .filter(|movie| !Self::is_presale(movie))
            .map(|movie| Repertoire {
                title: Self::parse_title(&movie),
                genres: Self::parse_genres(&movie),
                play_length: Self::parse_play_length(&movie),
                original_language: Self::parse_original_language(&movie),
                play_details: Self::parse_play_details(&movie),
            })
            .collect())
    }

    async fn fetch_venues(&self) -> AppResult<Vec<CinemaVenue>> {
        let rendered_html = self
            .render_with_retry(&self.cinema_venues_url, CINEMA_VENUES_PAGE_READY_SELECTOR)
            .await?;
        let html = Html::parse_document(&rendered_html);
        let legacy_venues = Self::parse_legacy_venues(&html);
        if !legacy_venues.is_empty() {
            return Ok(legacy_venues);
        }

        Self::parse_api_sites_list_venues(&rendered_html)
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

fn selector(value: &str) -> &'static Selector {
    Box::leak(Box::new(Selector::parse(value).expect("selector must compile")))
}

fn extract_json_array_assignment<'a>(html: &'a str, variable_name: &str) -> Option<&'a str> {
    let start = html.find(&format!("{variable_name} = ["))?;
    let array_start = start + html[start..].find('[')?;
    let mut depth = 0;
    let mut inside_string = false;
    let mut escaped = false;

    for (offset, character) in html[array_start..].char_indices() {
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
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    let array_end = array_start + offset + character.len_utf8();
                    return Some(&html[array_start..array_end]);
                }
            }
            _ => {}
        }
    }

    None
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

    loop {
        if !is_first_attempt && Instant::now() >= deadline {
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
                    return Ok(rendered_html);
                }
            }
            Err(error) if is_transient_browser_error(&error) => {
                last_transient_error = Some(error);
            }
            Err(error) => return Err(error),
        }

        if Instant::now() >= deadline {
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

fn classify_render_error(error: AppError) -> RetryDirective<AppError> {
    match error {
        error @ AppError::BrowserUnavailable(_) => RetryDirective::retry(error),
        error => RetryDirective::fail(error),
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
