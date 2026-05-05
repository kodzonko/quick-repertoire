use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};

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
static SELECTOR_CACHE: LazyLock<RwLock<HashMap<String, Arc<Selector>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

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

pub fn selector(value: &str) -> Arc<Selector> {
    if let Some(cached) =
        SELECTOR_CACHE.read().expect("selector cache read lock poisoned").get(value).cloned()
    {
        return cached;
    }

    let parsed = Arc::new(Selector::parse(value).expect("selector must compile"));
    let mut cache = SELECTOR_CACHE.write().expect("selector cache write lock poisoned");
    cache.entry(value.to_string()).or_insert_with(|| parsed.clone()).clone()
}

pub fn first_text(element: &ElementRef<'_>, selector_value: &str) -> Option<String> {
    let selector = selector(selector_value);
    element.select(selector.as_ref()).next().map(normalized_text)
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

pub fn extract_json_array_assignment<'a>(html: &'a str, variable_name: &str) -> Option<&'a str> {
    extract_json_assignment(html, variable_name, '[', ']')
}

pub fn extract_json_object_assignment<'a>(html: &'a str, variable_name: &str) -> Option<&'a str> {
    extract_json_assignment(html, variable_name, '{', '}')
}

pub fn extract_json_assignment<'a>(
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

pub fn fold_polish_character_to_ascii(character: char) -> char {
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
    use std::sync::Arc;

    use super::selector;

    #[test]
    fn selector_reuses_cached_instance_for_identical_values() {
        let first = selector("div.example");
        let second = selector("div.example");

        assert!(Arc::ptr_eq(&first, &second));
    }
}
