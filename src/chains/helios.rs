use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use scraper::Html;
use serde::Deserialize;

use crate::cinema::browser::{
    BrowserEvaluation, HtmlRenderer, render_html_with_retry, render_page_with_retry,
};
use crate::cinema::common::{MISSING_DATA_LABEL, normalize_lookup_text, normalized_text, selector};
use crate::cinema::registry::CinemaChainClient;
use crate::domain::{
    CinemaChainId, CinemaVenue, MovieLookupMetadata, MoviePlayDetails, MoviePlayTime, Repertoire,
};
use crate::error::{AppError, AppResult};
use crate::retry::RetryPolicy;

pub const DEFAULT_HELIOS_BASE_URL: &str = "https://helios.pl";
pub const DEFAULT_HELIOS_VENUES_URL: &str = "https://helios.pl/";

const VENUES_PAGE_READY_SELECTOR: &str = "body";
const REPERTOIRE_PAGE_READY_SELECTOR: &str = "section[control=\"repertoire-listing\"]";
const VENUE_LINK_SELECTOR: &str = "a[href^='/'][href*='/kino-helios']";
const REPERTOIRE_STATE_EVALUATION_NAME: &str = "repertoire";
const CURRENT_CINEMA_EVALUATION_NAME: &str = "current_cinema";

#[derive(Clone)]
pub struct Helios {
    base_url: String,
    venues_url: String,
    renderer: Arc<dyn HtmlRenderer>,
    retry_policy: RetryPolicy,
}

impl Helios {
    pub fn new(
        base_url: impl Into<String>,
        venues_url: impl Into<String>,
        renderer: Arc<dyn HtmlRenderer>,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            venues_url: venues_url.into(),
            renderer,
            retry_policy: RetryPolicy::network_requests(),
        }
    }

    pub fn with_retry_policy(mut self, retry_policy: RetryPolicy) -> Self {
        self.retry_policy = retry_policy;
        self
    }

    fn build_repertoire_url(&self, venue_id: &str) -> String {
        format!("{}/{}/repertuar", self.base_url.trim_end_matches('/'), venue_id.trim_matches('/'))
    }

    fn parse_venues(rendered_html: &str) -> Vec<CinemaVenue> {
        let html = Html::parse_document(rendered_html);
        let selector = selector(VENUE_LINK_SELECTOR);
        let mut seen_ids = HashSet::new();
        let mut venues = Vec::new();

        for anchor in html.select(selector.as_ref()) {
            let Some(href) = anchor.value().attr("href") else {
                continue;
            };
            let venue_id = href.trim().trim_matches('/').to_string();
            if venue_id.is_empty() || !seen_ids.insert(venue_id.clone()) {
                continue;
            }

            let venue_name = normalize_anchor_label(anchor);
            if venue_name.is_empty() {
                continue;
            }

            venues.push(CinemaVenue {
                chain_id: CinemaChainId::Helios.as_str().to_string(),
                venue_id,
                venue_name,
            });
        }

        venues
    }

    fn parse_repertoire(
        date: &str,
        base_url: &str,
        route_fragment: &str,
        repertoire_url: &str,
        repertoire_state: HeliosRepertoireState,
    ) -> Vec<Repertoire> {
        let Some(entries_for_date) = repertoire_state.screenings.get(date) else {
            return Vec::new();
        };

        let mut repertoire = Vec::new();
        for catalog_item in repertoire_state.list {
            let entry_key = entry_key_for(&catalog_item);
            let Some(entry) = entries_for_date.get(&entry_key) else {
                continue;
            };
            if entry.screenings.is_empty() {
                continue;
            }

            let display_source = if catalog_item.is_event {
                entry
                    .first_event()
                    .map(HeliosDisplaySource::from)
                    .unwrap_or_else(|| HeliosDisplaySource::from(&catalog_item))
            } else {
                HeliosDisplaySource::from(&catalog_item)
            };
            let lookup_source = if catalog_item.is_event {
                entry
                    .first_underlying_movie()
                    .map(HeliosLookupSource::from)
                    .or_else(|| entry.first_event().map(HeliosLookupSource::from))
                    .unwrap_or_else(|| HeliosLookupSource::from(&catalog_item))
            } else {
                HeliosLookupSource::from(&catalog_item)
            };

            let play_details = Self::build_play_details(
                &entry.screenings,
                repertoire_url,
                &display_source.source_id,
                display_source.id,
                lookup_source.is_imax,
            );
            if play_details.is_empty() {
                continue;
            }

            repertoire.push(Repertoire {
                title: display_source.title.clone(),
                genres: join_or_missing(&display_source.genres),
                play_length: display_source
                    .duration
                    .map(|duration| format!("{duration} min"))
                    .unwrap_or_else(|| MISSING_DATA_LABEL.to_string()),
                original_language: MISSING_DATA_LABEL.to_string(),
                play_details,
                lookup_metadata: build_lookup_metadata(
                    &display_source,
                    &lookup_source,
                    base_url,
                    route_fragment,
                ),
            });
        }

        repertoire
    }

    fn build_play_details(
        screenings: &[HeliosDayScreening],
        repertoire_url: &str,
        item_source_id: &str,
        item_id: u32,
        is_imax: bool,
    ) -> Vec<MoviePlayDetails> {
        let mut screenings = screenings.to_vec();
        screenings.sort_by(|left, right| left.time_from.cmp(&right.time_from));

        let mut play_details = Vec::<MoviePlayDetails>::new();
        for screening in screenings {
            let Some(play_time_value) = extract_showtime_value(&screening.time_from) else {
                continue;
            };
            let movie_print = screening.effective_movie_print();
            let format = build_format(is_imax, &screening.cinema_screen, &movie_print);
            let play_language = movie_print
                .speaking_type_label
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| MISSING_DATA_LABEL.to_string());
            let play_time = MoviePlayTime {
                value: play_time_value,
                url: Some(build_booking_url(
                    repertoire_url,
                    &screening.source_id,
                    &screening.cinema_source_id,
                    item_source_id,
                    item_id,
                )),
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

        play_details
    }
}

#[async_trait]
impl CinemaChainClient for Helios {
    async fn fetch_repertoire(
        &self,
        date: &str,
        venue: &CinemaVenue,
    ) -> AppResult<Vec<Repertoire>> {
        let repertoire_url = self.build_repertoire_url(&venue.venue_id);
        let rendered_page = render_page_with_retry(
            self.renderer.as_ref(),
            self.retry_policy,
            &repertoire_url,
            REPERTOIRE_PAGE_READY_SELECTOR,
            &[
                BrowserEvaluation::new(
                    REPERTOIRE_STATE_EVALUATION_NAME,
                    "JSON.stringify(window.__NUXT__.state.repertoire)",
                ),
                BrowserEvaluation::new(
                    CURRENT_CINEMA_EVALUATION_NAME,
                    "JSON.stringify(window.__NUXT__.state.cinema || window.__NUXT__.state.core.current)",
                ),
            ],
        )
        .await?;

        let repertoire_state = parse_required_json::<HeliosRepertoireState>(
            &rendered_page,
            REPERTOIRE_STATE_EVALUATION_NAME,
        )?;
        let current_cinema = parse_required_json::<HeliosCurrentCinema>(
            &rendered_page,
            CURRENT_CINEMA_EVALUATION_NAME,
        )?;
        let route_fragment = current_cinema.route_fragment();

        Ok(Self::parse_repertoire(
            date,
            &self.base_url,
            &route_fragment,
            &format!("{}/{}/repertuar", self.base_url.trim_end_matches('/'), route_fragment),
            repertoire_state,
        ))
    }

    async fn fetch_venues(&self) -> AppResult<Vec<CinemaVenue>> {
        let rendered_html = render_html_with_retry(
            self.renderer.as_ref(),
            self.retry_policy,
            &self.venues_url,
            VENUES_PAGE_READY_SELECTOR,
        )
        .await?;
        Ok(Self::parse_venues(&rendered_html))
    }
}

#[derive(Debug, Deserialize)]
struct HeliosRepertoireState {
    #[serde(default)]
    list: Vec<HeliosCatalogItem>,
    #[serde(default)]
    screenings: HashMap<String, HashMap<String, HeliosDateEntry>>,
}

#[derive(Debug, Deserialize, Clone)]
struct HeliosCatalogItem {
    id: u32,
    #[serde(rename = "sourceId")]
    source_id: String,
    title: String,
    #[serde(rename = "titleOriginal")]
    title_original: Option<String>,
    slug: String,
    duration: Option<u16>,
    #[serde(default)]
    genres: Vec<HeliosNamedValue>,
    #[serde(rename = "premiereDate")]
    premiere_date: Option<String>,
    #[serde(rename = "cinemaPremiereDate")]
    cinema_premiere_date: Option<String>,
    #[serde(rename = "isEvent", default)]
    is_event: bool,
    #[serde(rename = "isImax", default)]
    is_imax: bool,
}

#[derive(Debug, Deserialize, Clone, Default)]
struct HeliosDateEntry {
    #[serde(default)]
    screenings: Vec<HeliosDayScreening>,
}

impl HeliosDateEntry {
    fn first_event(&self) -> Option<&HeliosEventInfo> {
        self.screenings.iter().find_map(|screening| screening.event.as_ref())
    }

    fn first_underlying_movie(&self) -> Option<&HeliosNestedMovie> {
        self.screenings
            .iter()
            .flat_map(|screening| screening.screening_movies.iter())
            .map(|screening_movie| &screening_movie.movie)
            .next()
    }
}

#[derive(Debug, Deserialize, Clone)]
struct HeliosDayScreening {
    #[serde(rename = "timeFrom")]
    time_from: String,
    #[serde(rename = "sourceId")]
    source_id: String,
    #[serde(rename = "cinemaSourceId")]
    cinema_source_id: String,
    #[serde(rename = "cinemaScreen", default)]
    cinema_screen: HeliosCinemaScreen,
    #[serde(rename = "moviePrint", default)]
    movie_print: HeliosMoviePrint,
    event: Option<HeliosEventInfo>,
    #[serde(rename = "screeningMovies", default)]
    screening_movies: Vec<HeliosScreeningMovie>,
}

impl HeliosDayScreening {
    fn effective_movie_print(&self) -> HeliosMoviePrint {
        if self.movie_print.is_empty() {
            self.screening_movies
                .first()
                .map(|screening_movie| screening_movie.movie_print.clone())
                .unwrap_or_default()
        } else {
            self.movie_print.clone()
        }
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
struct HeliosCinemaScreen {
    #[serde(default)]
    feature: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
struct HeliosMoviePrint {
    #[serde(rename = "printType", default)]
    print_type: String,
    #[serde(rename = "printRelease", default)]
    print_release: String,
    #[serde(rename = "soundType", default)]
    sound_type: String,
    #[serde(rename = "speakingTypeLabel")]
    speaking_type_label: Option<String>,
}

impl HeliosMoviePrint {
    fn is_empty(&self) -> bool {
        self.print_type.trim().is_empty()
            && self.print_release.trim().is_empty()
            && self.sound_type.trim().is_empty()
            && self.speaking_type_label.as_deref().unwrap_or_default().trim().is_empty()
    }
}

#[derive(Debug, Deserialize, Clone)]
struct HeliosScreeningMovie {
    movie: HeliosNestedMovie,
    #[serde(rename = "moviePrint", default)]
    movie_print: HeliosMoviePrint,
}

#[derive(Debug, Deserialize, Clone)]
struct HeliosEventInfo {
    id: u32,
    #[serde(rename = "sourceId")]
    source_id: String,
    name: String,
    slug: String,
    duration: Option<u16>,
    #[serde(default)]
    genres: Vec<HeliosNamedValue>,
    #[serde(rename = "isImax", default)]
    is_imax: bool,
}

#[derive(Debug, Deserialize, Clone)]
struct HeliosNestedMovie {
    id: u32,
    #[serde(rename = "sourceId")]
    source_id: String,
    title: String,
    #[serde(rename = "titleOriginal")]
    title_original: Option<String>,
    slug: String,
    duration: Option<u16>,
    #[serde(default)]
    genres: Vec<HeliosNamedValue>,
    #[serde(rename = "premiereDate")]
    premiere_date: Option<String>,
    #[serde(rename = "cinemaPremiereDate")]
    cinema_premiere_date: Option<String>,
    #[serde(rename = "isImax", default)]
    is_imax: bool,
}

#[derive(Debug, Deserialize)]
struct HeliosCurrentCinema {
    #[serde(rename = "slugCity")]
    slug_city: String,
    slug: String,
}

impl HeliosCurrentCinema {
    fn route_fragment(&self) -> String {
        format!("{}/{}", self.slug_city.trim_matches('/'), self.slug.trim_matches('/'))
    }
}

#[derive(Debug, Deserialize, Clone)]
struct HeliosNamedValue {
    name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeliosPageKind {
    Movie,
    Event,
}

#[derive(Debug, Clone)]
struct HeliosDisplaySource {
    id: u32,
    source_id: String,
    title: String,
    duration: Option<u16>,
    genres: Vec<String>,
}

impl From<&HeliosCatalogItem> for HeliosDisplaySource {
    fn from(value: &HeliosCatalogItem) -> Self {
        Self {
            id: value.id,
            source_id: value.source_id.clone(),
            title: value.title.clone(),
            duration: value.duration,
            genres: value.genres.iter().map(|genre| genre.name.clone()).collect(),
        }
    }
}

impl From<&HeliosEventInfo> for HeliosDisplaySource {
    fn from(value: &HeliosEventInfo) -> Self {
        Self {
            id: value.id,
            source_id: value.source_id.clone(),
            title: value.name.clone(),
            duration: value.duration,
            genres: value.genres.iter().map(|genre| genre.name.clone()).collect(),
        }
    }
}

#[derive(Debug, Clone)]
struct HeliosLookupSource {
    id: u32,
    source_id: String,
    title: String,
    title_original: Option<String>,
    slug: String,
    duration: Option<u16>,
    genres: Vec<String>,
    premiere_date: Option<String>,
    cinema_premiere_date: Option<String>,
    is_imax: bool,
    page_kind: HeliosPageKind,
}

impl From<&HeliosCatalogItem> for HeliosLookupSource {
    fn from(value: &HeliosCatalogItem) -> Self {
        Self {
            id: value.id,
            source_id: value.source_id.clone(),
            title: value.title.clone(),
            title_original: value.title_original.clone(),
            slug: value.slug.clone(),
            duration: value.duration,
            genres: value.genres.iter().map(|genre| genre.name.clone()).collect(),
            premiere_date: value.premiere_date.clone(),
            cinema_premiere_date: value.cinema_premiere_date.clone(),
            is_imax: value.is_imax,
            page_kind: if value.is_event { HeliosPageKind::Event } else { HeliosPageKind::Movie },
        }
    }
}

impl From<&HeliosEventInfo> for HeliosLookupSource {
    fn from(value: &HeliosEventInfo) -> Self {
        Self {
            id: value.id,
            source_id: value.source_id.clone(),
            title: value.name.clone(),
            title_original: None,
            slug: value.slug.clone(),
            duration: value.duration,
            genres: value.genres.iter().map(|genre| genre.name.clone()).collect(),
            premiere_date: None,
            cinema_premiere_date: None,
            is_imax: value.is_imax,
            page_kind: HeliosPageKind::Event,
        }
    }
}

impl From<&HeliosNestedMovie> for HeliosLookupSource {
    fn from(value: &HeliosNestedMovie) -> Self {
        Self {
            id: value.id,
            source_id: value.source_id.clone(),
            title: value.title.clone(),
            title_original: value.title_original.clone(),
            slug: value.slug.clone(),
            duration: value.duration,
            genres: value.genres.iter().map(|genre| genre.name.clone()).collect(),
            premiere_date: value.premiere_date.clone(),
            cinema_premiere_date: value.cinema_premiere_date.clone(),
            is_imax: value.is_imax,
            page_kind: HeliosPageKind::Movie,
        }
    }
}

fn normalize_anchor_label(anchor: scraper::ElementRef<'_>) -> String {
    normalize_whitespace(
        &normalized_text(anchor).replace("Sala Dream", "").replace("Sala Imax", ""),
    )
}

fn entry_key_for(item: &HeliosCatalogItem) -> String {
    let prefix = if item.is_event { 'e' } else { 'm' };
    format!("{prefix}{}", item.id)
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_required_json<T: for<'de> Deserialize<'de>>(
    rendered_page: &crate::cinema::browser::RenderedPage,
    evaluation_name: &str,
) -> AppResult<T> {
    let payload = rendered_page.evaluation(evaluation_name).ok_or_else(|| {
        AppError::BrowserUnavailable(format!(
            "Brak wyniku skryptu `{evaluation_name}` podczas odczytu danych Helios."
        ))
    })?;
    serde_json::from_str(payload).map_err(|error| {
        AppError::BrowserUnavailable(format!(
            "Nie udało się odczytać danych Helios z wyniku `{evaluation_name}`: {error}"
        ))
    })
}

fn build_lookup_metadata(
    display_source: &HeliosDisplaySource,
    lookup_source: &HeliosLookupSource,
    base_url: &str,
    route_fragment: &str,
) -> MovieLookupMetadata {
    let mut alternate_titles = Vec::new();
    push_alternate_title(&mut alternate_titles, &display_source.title, Some(&lookup_source.title));
    push_alternate_title(
        &mut alternate_titles,
        &display_source.title,
        lookup_source.title_original.as_deref(),
    );

    MovieLookupMetadata {
        chain_movie_id: Some(lookup_source.source_id.clone()),
        movie_page_url: Some(build_item_page_url(
            base_url,
            route_fragment,
            lookup_source.page_kind,
            &lookup_source.slug,
            lookup_source.id,
        )),
        alternate_titles,
        runtime_minutes: lookup_source.duration,
        original_language_code: None,
        genre_tags: normalize_genre_tags(&lookup_source.genres),
        production_year: lookup_source.premiere_date.as_deref().and_then(parse_year_from_date),
        polish_premiere_date: lookup_source
            .cinema_premiere_date
            .clone()
            .or_else(|| lookup_source.premiere_date.clone()),
    }
}

fn push_alternate_title(
    alternate_titles: &mut Vec<String>,
    base_title: &str,
    candidate: Option<&str>,
) {
    let Some(candidate) = candidate.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };
    let normalized_candidate = normalize_lookup_text(candidate);
    if normalized_candidate.is_empty()
        || normalize_lookup_text(base_title) == normalized_candidate
        || alternate_titles
            .iter()
            .any(|existing| normalize_lookup_text(existing) == normalized_candidate)
    {
        return;
    }
    alternate_titles.push(candidate.to_string());
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

fn build_item_page_url(
    base_url: &str,
    route_fragment: &str,
    page_kind: HeliosPageKind,
    slug: &str,
    id: u32,
) -> String {
    let path_segment = match page_kind {
        HeliosPageKind::Movie => "filmy",
        HeliosPageKind::Event => "wydarzenie",
    };
    format!(
        "{}/{}/{path_segment}/{}-{}",
        base_url.trim_end_matches('/'),
        route_fragment.trim_matches('/'),
        slug,
        id,
    )
}

fn build_format(
    is_imax: bool,
    cinema_screen: &HeliosCinemaScreen,
    movie_print: &HeliosMoviePrint,
) -> String {
    let mut parts = Vec::new();
    let feature = cinema_screen.feature.trim().to_ascii_lowercase();
    if feature.contains("imax") || is_imax {
        parts.push("IMAX".to_string());
    }
    if feature.contains("dream") {
        parts.push("Dream".to_string());
    }

    let print_type = movie_print.print_type.trim();
    if !print_type.is_empty() && !parts.iter().any(|part| part.eq_ignore_ascii_case(print_type)) {
        parts.push(print_type.to_string());
    }

    if has_atmos(movie_print) && !parts.iter().any(|part| part.eq_ignore_ascii_case("Atmos")) {
        parts.push("Atmos".to_string());
    }

    if parts.is_empty() { MISSING_DATA_LABEL.to_string() } else { parts.join(" ") }
}

fn has_atmos(movie_print: &HeliosMoviePrint) -> bool {
    movie_print.sound_type.eq_ignore_ascii_case("ATM")
        || movie_print.print_release.to_ascii_uppercase().contains("ATMOS")
}

fn extract_showtime_value(time_from: &str) -> Option<String> {
    time_from.get(11..16).map(str::to_string)
}

fn build_booking_url(
    repertoire_url: &str,
    screening_source_id: &str,
    cinema_source_id: &str,
    item_source_id: &str,
    item_id: u32,
) -> String {
    format!(
        "https://bilety.helios.pl/screen/{screening_source_id}?cinemaId={cinema_source_id}&backUrl={}&item_id={item_source_id}&item_source_id={item_id}",
        percent_encode_query_value(repertoire_url),
    )
}

fn percent_encode_query_value(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        let character = byte as char;
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '~') {
            encoded.push(character);
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{byte:02X}"));
        }
    }
    encoded
}

fn parse_year_from_date(value: &str) -> Option<i32> {
    value.get(0..4)?.parse::<i32>().ok()
}

fn join_or_missing(values: &[String]) -> String {
    if values.is_empty() { MISSING_DATA_LABEL.to_string() } else { values.join(", ") }
}
