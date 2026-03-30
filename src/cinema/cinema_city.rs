use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use async_trait::async_trait;
use chromiumoxide::browser::{Browser, BrowserConfig};
use futures::StreamExt;
use regex::Regex;
use scraper::{ElementRef, Html, Selector};

use crate::cinema::registry::CinemaChainClient;
use crate::domain::{CinemaChainId, CinemaVenue, MoviePlayDetails, Repertoire};
use crate::error::{AppError, AppResult};

const REQUEST_TIMEOUT_SECONDS: u64 = 30;
const REPERTOIRE_SELECTOR: &str = "div.row.qb-movie";
const CINEMA_VENUES_SELECTOR: &str = "option[value][data-tokens]";

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
        page.find_element(wait_selector)
            .await
            .map_err(|error| AppError::BrowserUnavailable(error.to_string()))?;
        let html = page
            .content()
            .await
            .map_err(|error| AppError::BrowserUnavailable(error.to_string()))?;
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
}

impl CinemaCity {
    pub fn new(
        repertoire_url: String,
        cinema_venues_url: String,
        renderer: Arc<dyn HtmlRenderer>,
    ) -> Self {
        Self { repertoire_url, cinema_venues_url, renderer }
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
        first_text(movie, "h3.qb-movie-name").unwrap_or_else(|| "N/A".to_string())
    }

    fn parse_genres(movie: &ElementRef<'_>) -> String {
        let raw = first_text(movie, "div.qb-movie-info-wrapper span")
            .unwrap_or_else(|| "N/A".to_string());
        if raw.contains('|') { raw.replace('|', "").trim().to_string() } else { "N/A".to_string() }
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
            .unwrap_or_else(|| "N/A".to_string())
    }

    fn parse_play_length(movie: &ElementRef<'_>) -> String {
        movie
            .select(selector("div.qb-movie-info-wrapper span"))
            .map(normalized_text)
            .find(|text| PLAY_LENGTH_RE.is_match(text))
            .unwrap_or_else(|| "N/A".to_string())
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
        if formats.is_empty() { "N/A".to_string() } else { formats.join(" ") }
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
                    label.contains("subbed-lang") || label.contains("dubbed-lang")
                })
            })
            .map(normalized_text);
        match prefix {
            Some(prefix) if language.as_ref().is_some_and(|value| !value.is_empty()) => {
                format!("{prefix}: {}", language.unwrap_or_default())
            }
            Some(prefix) => prefix,
            None => "N/A".to_string(),
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
}

#[async_trait]
impl CinemaChainClient for CinemaCity {
    async fn fetch_repertoire(
        &self,
        date: &str,
        venue: &CinemaVenue,
    ) -> AppResult<Vec<Repertoire>> {
        let url = Self::fill_string_template(
            &self.repertoire_url,
            &[("cinema_venue_id", venue.venue_id.as_str()), ("repertoire_date", date)],
        )?;
        let rendered_html = self.renderer.render(&url, REPERTOIRE_SELECTOR).await?;
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
        let rendered_html =
            self.renderer.render(&self.cinema_venues_url, CINEMA_VENUES_SELECTOR).await?;
        let html = Html::parse_document(&rendered_html);
        Ok(html
            .select(selector(CINEMA_VENUES_SELECTOR))
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
            .collect())
    }
}

fn selector(value: &str) -> &'static Selector {
    Box::leak(Box::new(Selector::parse(value).expect("selector must compile")))
}

fn first_text(element: &ElementRef<'_>, selector_value: &str) -> Option<String> {
    element.select(selector(selector_value)).next().map(normalized_text)
}

fn normalized_text(element: ElementRef<'_>) -> String {
    WHITESPACE_RE.replace_all(&element.text().collect::<String>(), " ").trim().to_string()
}
