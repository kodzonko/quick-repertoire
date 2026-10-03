use std::sync::LazyLock;

use log::debug;
use regex::Regex;
use scraper::{ElementRef, Selector};
use serde::Deserialize;

use crate::domain::MoviePageFallbackDetails;
use crate::error::{AppError, AppResult};
use crate::logging::preview_for_log;

const MAX_LOG_BODY_PREVIEW_CHARS: usize = 256;

pub const MISSING_DATA_LABEL: &str = "Brak danych";

static WHITESPACE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+").expect("whitespace regex must compile"));
#[derive(Debug, Deserialize)]
struct EmbeddedMoviePageDetails {
    #[serde(rename = "originalName")]
    original_name: Option<String>,
    #[serde(rename = "releaseCountry")]
    release_country: Option<String>,
    cast: Option<String>,
    directors: Option<String>,
    synopsis: Option<String>,
}

pub fn selector(value: &str) -> Selector {
    Selector::parse(value).expect("selector must compile")
}

pub fn first_text(element: &ElementRef<'_>, selector_value: &str) -> Option<String> {
    let selector = selector(selector_value);
    element.select(&selector).next().map(normalized_text)
}

pub fn normalized_text(element: ElementRef<'_>) -> String {
    WHITESPACE_RE.replace_all(&element.text().collect::<String>(), " ").trim().to_string()
}

pub fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.map(|value| value.trim().to_string()).filter(|value| !value.is_empty())
}

pub fn split_people_list(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split([',', ';'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

pub fn extract_query_param(url: &str, parameter_name: &str) -> Option<String> {
    url.split(['?', '#', '&'])
        .filter_map(|segment| segment.split_once('='))
        .find(|(name, _)| *name == parameter_name)
        .map(|(_, value)| value.to_string())
        .filter(|value| !value.trim().is_empty())
}

pub fn extract_json_array_assignment<'a>(
    html: &'a str,
    variable_name: &str,
) -> Result<Option<&'a str>, serde_json::Error> {
    extract_json_assignment(html, variable_name, '[')
}

pub fn extract_json_object_assignment<'a>(
    html: &'a str,
    variable_name: &str,
) -> Result<Option<&'a str>, serde_json::Error> {
    extract_json_assignment(html, variable_name, '{')
}

fn extract_json_assignment<'a>(
    html: &'a str,
    variable_name: &str,
    open_char: char,
) -> Result<Option<&'a str>, serde_json::Error> {
    let Some(start) = html.find(&format!("{variable_name} = {open_char}")) else {
        return Ok(None);
    };
    let json_start = start + html[start..].find(open_char).expect("assignment contains JSON start");
    let mut values =
        serde_json::Deserializer::from_str(&html[json_start..]).into_iter::<serde_json::Value>();
    match values.next() {
        Some(Ok(_)) => Ok(Some(&html[json_start..json_start + values.byte_offset()])),
        Some(Err(error)) => Err(error),
        None => Ok(None),
    }
}

pub fn fold_polish_character_to_ascii(character: char) -> char {
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

pub fn normalize_lookup_text(value: &str) -> String {
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

pub fn parse_movie_page_fallback_details(
    rendered_html: &str,
) -> AppResult<MoviePageFallbackDetails> {
    let Some(film_details_json) = extract_json_object_assignment(rendered_html, "filmDetails")
        .map_err(|error| {
            AppError::BrowserUnavailable(format!(
                "Nie udało się odczytać szczegółów filmu z aktualnego formatu strony: {error}"
            ))
        })?
    else {
        debug!(
            "Movie page did not include a filmDetails assignment; html_preview={}",
            preview_for_log(rendered_html, MAX_LOG_BODY_PREVIEW_CHARS),
        );
        return Err(AppError::BrowserUnavailable(
            "Nie udało się odczytać szczegółów filmu z aktualnego formatu strony.".to_string(),
        ));
    };

    let details =
        serde_json::from_str::<EmbeddedMoviePageDetails>(film_details_json).map_err(|error| {
            debug!(
                "Movie page filmDetails JSON parse failed error={error} payload_preview={}",
                preview_for_log(film_details_json, MAX_LOG_BODY_PREVIEW_CHARS),
            );
            AppError::BrowserUnavailable(format!(
                "Nie udało się odczytać szczegółów filmu z aktualnego formatu strony: {error}"
            ))
        })?;

    Ok(MoviePageFallbackDetails {
        original_title: normalize_optional_text(details.original_name),
        country: normalize_optional_text(details.release_country),
        cast: split_people_list(details.cast.as_deref()),
        directors: split_people_list(details.directors.as_deref()),
        synopsis: normalize_optional_text(details.synopsis),
    })
}

#[cfg(test)]
mod tests {
    use super::{extract_json_object_assignment, normalize_lookup_text};

    #[test]
    fn extracts_embedded_json_with_escaped_quotes_and_braces() {
        let html = r#"filmDetails = {"synopsis":"escaped \" brace }","cast":["A"]}; next"#;
        assert_eq!(
            extract_json_object_assignment(html, "filmDetails").unwrap(),
            Some(r#"{"synopsis":"escaped \" brace }","cast":["A"]}"#),
        );
    }

    #[test]
    fn normalizes_accented_titles() {
        assert_eq!(normalize_lookup_text("Żółć — Café Čas"), "zolc cafe cas");
    }
}
