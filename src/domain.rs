use std::fmt::{Display, Formatter};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CinemaChainId {
    CinemaCity,
}

impl CinemaChainId {
    pub fn from_value(value: &str) -> AppResult<Self> {
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "cinema-city" => Ok(Self::CinemaCity),
            _ => Err(AppError::UnsupportedCinemaChain {
                invalid_chain: value.to_string(),
                supported_chains: Self::supported_values().join(", "),
            }),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::CinemaCity => "cinema-city",
        }
    }

    pub fn section_name(self) -> &'static str {
        match self {
            Self::CinemaCity => "cinema_city",
        }
    }

    pub fn supported_values() -> Vec<&'static str> {
        vec![Self::CinemaCity.as_str()]
    }
}

impl Display for CinemaChainId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CinemaVenue {
    pub chain_id: String,
    pub venue_id: String,
    pub venue_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoviePlayTime {
    pub value: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoviePlayDetails {
    pub format: String,
    pub play_language: String,
    pub play_times: Vec<MoviePlayTime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MovieLookupMetadata {
    pub cinema_city_film_id: Option<String>,
    pub movie_page_url: Option<String>,
    pub alternate_titles: Vec<String>,
    pub runtime_minutes: Option<u16>,
    pub original_language_code: Option<String>,
    pub genre_tags: Vec<String>,
    pub production_year: Option<i32>,
    pub polish_premiere_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MoviePageFallbackDetails {
    pub original_title: Option<String>,
    pub country: Option<String>,
    pub cast: Vec<String>,
    pub directors: Vec<String>,
    pub synopsis: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repertoire {
    pub title: String,
    pub genres: String,
    pub play_length: String,
    pub original_language: String,
    pub play_details: Vec<MoviePlayDetails>,
    pub lookup_metadata: MovieLookupMetadata,
}

impl Repertoire {
    pub fn tmdb_lookup_key(&self) -> &str {
        self.lookup_metadata.cinema_city_film_id.as_deref().unwrap_or(&self.title)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmdbLookupMovie {
    pub lookup_key: String,
    pub title: String,
    pub lookup_metadata: MovieLookupMetadata,
}

impl From<&Repertoire> for TmdbLookupMovie {
    fn from(value: &Repertoire) -> Self {
        Self {
            lookup_key: value.tmdb_lookup_key().to_string(),
            title: value.title.clone(),
            lookup_metadata: value.lookup_metadata.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepertoireCliTableMetadata {
    pub chain_display_name: String,
    pub repertoire_date: String,
    pub cinema_venue_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmdbMovieDetails {
    pub rating: String,
    pub summary: String,
}
