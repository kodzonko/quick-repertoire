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
                    .map(|play| {
                        format!(
                            "[{}, {}]:\n{}",
                            play.format,
                            play.play_language,
                            play.play_times
                                .iter()
                                .map(render_play_time)
                                .collect::<Vec<_>>()
                                .join(" ")
                        )
                    })
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

    format!(
        "Repertuar dla {} ({}) na dzień: {}\n{}",
        table_metadata.chain_display_name,
        table_metadata.cinema_venue_name,
        table_metadata.repertoire_date,
        table
    )
}

fn render_play_time(play_time: &MoviePlayTime) -> String {
    let Some(url) = play_time.url.as_deref().and_then(sanitize_hyperlink_url) else {
        return play_time.value.clone();
    };

    format!("{OSC_8_PREFIX}{url}{OSC_8_SUFFIX}{}{OSC_8_PREFIX}{OSC_8_SUFFIX}", play_time.value)
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
}
