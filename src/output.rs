use std::collections::HashMap;
use std::sync::LazyLock;

use chrono::{Duration, Local, NaiveDate};
use comfy_table::{Cell, ContentArrangement, Table, presets::UTF8_FULL};
use regex::Regex;

use crate::domain::{
    CinemaVenue, MoviePlayTime, Repertoire, RepertoireCliTableMetadata, TmdbMovieDetails,
};
use crate::error::{AppError, AppResult};

const MISSING_DATA_LABEL: &str = "Brak danych";

static NON_WORD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\W").expect("non-word regex must compile"));
static WHITESPACE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+").expect("whitespace regex must compile"));
static NON_ASCII_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[^\x00-\x7F]").expect("non-ascii regex must compile"));
static MULTI_WILDCARD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"%{2,}").expect("wildcard regex must compile"));
const OSC_8_PREFIX: &str = "\u{1b}]8;;";
const OSC_8_SUFFIX: &str = "\u{1b}\\";
const HYPERLINK_MARKER_PREFIX: char = '\u{2060}';
const HYPERLINK_MARKER_ZERO: char = '\u{200C}';
const HYPERLINK_MARKER_ONE: char = '\u{200D}';
const HYPERLINK_MARKER_SUFFIX: char = '\u{2063}';

#[derive(Debug, Clone, PartialEq, Eq)]
struct HyperlinkReplacement {
    placeholder: String,
    rendered: String,
}

pub trait Terminal {
    fn write_line(&mut self, text: &str);
}

#[derive(Default)]
pub struct BufferTerminal {
    output: String,
}

impl BufferTerminal {
    pub fn into_string(self) -> String {
        self.output
    }
}

impl Terminal for BufferTerminal {
    fn write_line(&mut self, text: &str) {
        self.output.push_str(text);
        if !text.ends_with('\n') {
            self.output.push('\n');
        }
    }
}

pub struct StdoutTerminal;

impl Terminal for StdoutTerminal {
    fn write_line(&mut self, text: &str) {
        println!("{text}");
    }
}

pub fn cinema_venue_input_parser(cinema_venue: &str) -> String {
    let trimmed_outer_whitespaces = cinema_venue.trim();
    let non_letters_removed = NON_WORD_RE.replace_all(trimmed_outer_whitespaces, " ");
    let whitespaces_trimmed = WHITESPACE_RE.replace_all(&non_letters_removed, ",");
    let nonascii_removed = NON_ASCII_RE.replace_all(&whitespaces_trimmed, "_");
    let surrounding_wildcards_added = format!("%{}%", nonascii_removed.replace(',', "%"));
    MULTI_WILDCARD_RE.replace_all(&surrounding_wildcards_added, "%").to_string()
}

pub fn date_input_parser(date: &str) -> AppResult<String> {
    let trimmed = date.trim();
    let normalized = trimmed.to_lowercase();
    match normalized.as_str() {
        "dziś" | "dzis" | "dzisiaj" | "today" => Ok(Local::now().date_naive().to_string()),
        "jutro" | "tomorrow" => Ok((Local::now().date_naive() + Duration::days(1)).to_string()),
        _ => NaiveDate::parse_from_str(trimmed, "%Y-%m-%d")
            .map(|parsed| parsed.to_string())
            .map_err(|_| {
                AppError::Message(format!(
                    "Data: {trimmed} nie jest we wspieranym formacie: YYYY-MM-DD | dziś | jutro."
                ))
            }),
    }
}

pub fn render_venues_table(venues: &[CinemaVenue], chain_display_name: &str) -> String {
    if venues.is_empty() {
        return "Brak kin tej sieci w bazie danych.".to_string();
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["Nazwa lokalu", "ID lokalu"]);

    for venue in venues {
        table.add_row(vec![Cell::new(&venue.venue_name), Cell::new(&venue.venue_id)]);
    }

    format!("Znalezione lokale sieci {chain_display_name}\n{table}")
}

pub fn render_repertoire_table(
    repertoire: &[Repertoire],
    table_metadata: &RepertoireCliTableMetadata,
    ratings: &HashMap<String, TmdbMovieDetails>,
) -> String {
    render_repertoire_table_with_width(repertoire, table_metadata, ratings, None)
}

fn render_repertoire_table_with_width(
    repertoire: &[Repertoire],
    table_metadata: &RepertoireCliTableMetadata,
    ratings: &HashMap<String, TmdbMovieDetails>,
    table_width: Option<u16>,
) -> String {
    if repertoire.is_empty() {
        return "Brak repertuaru do wyświetlenia.".to_string();
    }

    let mut headers = vec!["Tytuł", "Gatunki", "Długość", "Język oryg.", "Seanse"];
    let show_ratings = !ratings.is_empty();
    if show_ratings {
        headers.push("Ocena z TMDB");
        headers.push("Opis z TMDB");
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(headers);
    if let Some(width) = table_width {
        table.set_width(width);
    }

    let mut hyperlink_replacements = Vec::new();

    for movie in repertoire {
        let mut row = vec![
            Cell::new(&movie.title),
            Cell::new(&movie.genres),
            Cell::new(&movie.play_length),
            Cell::new(&movie.original_language),
            Cell::new(
                movie
                    .play_details
                    .iter()
                    .map(|play| render_play_details(play, &mut hyperlink_replacements))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
        ];
        if show_ratings {
            let tmdb_details = ratings.get(&movie.title).cloned().unwrap_or(TmdbMovieDetails {
                rating: "0.0/10".to_string(),
                summary: MISSING_DATA_LABEL.to_string(),
            });
            row.push(Cell::new(tmdb_details.rating));
            row.push(Cell::new(tmdb_details.summary));
        }
        table.add_row(row);
    }

    apply_hyperlink_replacements(
        format!(
            "Repertuar dla {} ({}) na dzień: {}\n{}",
            table_metadata.chain_display_name,
            table_metadata.cinema_venue_name,
            table_metadata.repertoire_date,
            table
        ),
        &hyperlink_replacements,
    )
}

fn render_play_details(
    play_details: &crate::domain::MoviePlayDetails,
    hyperlink_replacements: &mut Vec<HyperlinkReplacement>,
) -> String {
    format!(
        "[{}, {}]:\n{}",
        play_details.format,
        play_details.play_language,
        play_details
            .play_times
            .iter()
            .map(|play_time| render_play_time_for_table(play_time, hyperlink_replacements))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

fn render_play_time_for_table(
    play_time: &MoviePlayTime,
    hyperlink_replacements: &mut Vec<HyperlinkReplacement>,
) -> String {
    if play_time.url.as_deref().and_then(sanitize_hyperlink_url).is_none() {
        return play_time.value.clone();
    }

    // `comfy-table` does not understand OSC-8 escape sequences, so layout must happen using
    // visible text only. We swap these zero-width placeholders back to hyperlinks afterwards.
    let placeholder = render_hyperlink_placeholder(&play_time.value, hyperlink_replacements.len());
    hyperlink_replacements.push(HyperlinkReplacement {
        placeholder: placeholder.clone(),
        rendered: render_play_time(play_time),
    });
    placeholder
}

fn apply_hyperlink_replacements(
    mut rendered_table: String,
    hyperlink_replacements: &[HyperlinkReplacement],
) -> String {
    for replacement in hyperlink_replacements {
        rendered_table = rendered_table.replace(&replacement.placeholder, &replacement.rendered);
    }

    rendered_table
}

fn render_hyperlink_placeholder(play_time_value: &str, index: usize) -> String {
    let marker = render_hyperlink_marker(index);
    format!("{marker}{play_time_value}{marker}")
}

fn render_hyperlink_marker(index: usize) -> String {
    let mut marker = String::new();
    marker.push(HYPERLINK_MARKER_PREFIX);

    if index == 0 {
        marker.push(HYPERLINK_MARKER_ZERO);
    } else {
        let mut remaining = index;
        while remaining > 0 {
            marker.push(if remaining.is_multiple_of(2) {
                HYPERLINK_MARKER_ZERO
            } else {
                HYPERLINK_MARKER_ONE
            });
            remaining /= 2;
        }
    }

    marker.push(HYPERLINK_MARKER_SUFFIX);
    marker
}

fn render_osc8_hyperlink(play_time_value: &str, url: &str) -> String {
    format!("{OSC_8_PREFIX}{url}{OSC_8_SUFFIX}{play_time_value}{OSC_8_PREFIX}{OSC_8_SUFFIX}")
}

fn render_play_time(play_time: &MoviePlayTime) -> String {
    let Some(url) = play_time.url.as_deref().and_then(sanitize_hyperlink_url) else {
        return play_time.value.clone();
    };

    render_osc8_hyperlink(&play_time.value, url)
}

fn sanitize_hyperlink_url(url: &str) -> Option<&str> {
    let trimmed = url.trim();
    if trimmed.is_empty()
        || trimmed.chars().any(|character| matches!(character, '\u{1b}' | '\n' | '\r'))
    {
        return None;
    }

    if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
        Some(trimmed)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::domain::{MoviePlayDetails, RepertoireCliTableMetadata};

    #[test]
    fn render_play_time_wraps_http_links_with_osc8_sequences() {
        let rendered = render_play_time(&MoviePlayTime {
            value: "10:00".to_string(),
            url: Some("https://www.cinema-city.pl/filmy/test-movie/123".to_string()),
        });

        assert_eq!(
            rendered,
            "\u{1b}]8;;https://www.cinema-city.pl/filmy/test-movie/123\u{1b}\\10:00\u{1b}]8;;\u{1b}\\"
        );
    }

    #[test]
    fn render_play_time_falls_back_to_plain_text_for_unsafe_links() {
        let rendered = render_play_time(&MoviePlayTime {
            value: "10:00".to_string(),
            url: Some("javascript:alert(1)".to_string()),
        });

        assert_eq!(rendered, "10:00");
    }

    #[test]
    fn render_repertoire_table_preserves_well_formed_hyperlinks_when_wrapping() {
        let metadata = RepertoireCliTableMetadata {
            chain_display_name: "Cinema City".to_string(),
            repertoire_date: "2026-04-01".to_string(),
            cinema_venue_name: "Wroclaw - Wroclavia".to_string(),
        };
        let repertoire = vec![Repertoire {
            title: "Test Movie".to_string(),
            genres: "Thriller".to_string(),
            play_length: "120 min".to_string(),
            original_language: "EN".to_string(),
            play_details: vec![MoviePlayDetails {
                format: "2D Projekcja Laserowa BARCO".to_string(),
                play_language: "FILM Z NAPISAMI: PL".to_string(),
                play_times: vec![MoviePlayTime {
                    value: "10:00".to_string(),
                    url: Some(format!(
                        "https://www.cinema-city.pl/filmy/test-movie/123/{}",
                        "a".repeat(200)
                    )),
                }],
            }],
        }];

        let rendered =
            render_repertoire_table_with_width(&repertoire, &metadata, &HashMap::new(), Some(80));
        let expected_hyperlink = format!(
            "\u{1b}]8;;https://www.cinema-city.pl/filmy/test-movie/123/{}\u{1b}\\10:00\u{1b}]8;;\u{1b}\\",
            "a".repeat(200)
        );

        assert!(rendered.contains(&expected_hyperlink));

        let well_formed_hyperlink_re =
            Regex::new(r"\x1b]8;;[^\x1b\r\n]+\x1b\\[^\x1b\r\n]+\x1b]8;;\x1b\\")
                .expect("well-formed OSC-8 regex must compile");
        let stripped = well_formed_hyperlink_re.replace_all(&rendered, "");

        assert!(!stripped.contains("]8;;"));
        assert!(!stripped.contains(HYPERLINK_MARKER_PREFIX));
        assert!(!stripped.contains(HYPERLINK_MARKER_ZERO));
        assert!(!stripped.contains(HYPERLINK_MARKER_ONE));
        assert!(!stripped.contains(HYPERLINK_MARKER_SUFFIX));
    }
}
