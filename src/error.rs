use std::path::Path;

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
    #[error("Nie znaleziono żadnego lokalu o podanej nazwie.")]
    VenueNotFound,
    #[error(
        "Podana nazwa lokalu jest niejednoznaczna. Znaleziono {matches_count} {noun}.",
        noun = ambiguous_noun(*matches_count)
    )]
    AmbiguousVenueMatch { matches_count: usize },
    #[error("Nieobsługiwana sieć kin: {invalid_chain}. Dostępne wartości: {supported_chains}.")]
    UnsupportedCinemaChain { invalid_chain: String, supported_chains: String },
    #[error("Brak domyślnego lokalu skonfigurowanego dla sieci {chain_display_name}.")]
    DefaultVenueNotConfigured { chain_display_name: String },
    #[error(
        "Nie znaleziono pliku konfiguracji: config.ini. Uruchom aplikację ponownie, aby przejść przez konfigurację początkową."
    )]
    ConfigurationNotFound,
    #[error("{0}")]
    Configuration(String),
    #[error("Konfiguracja została przerwana przez użytkownika.")]
    ConfigurationAborted,
    #[error(
        "Nie udało się wypełnić templatki z adresem url. Brakująca zmienna: {missing_variable}."
    )]
    TemplateRender { missing_variable: String },
    #[error("{0}")]
    DatabaseConnection(String),
    #[error("{0}")]
    BrowserUnavailable(String),
    #[error("{0}")]
    Http(String),
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration(message.into())
    }

    pub fn database_connection(path: &Path) -> Self {
        let sqlite_uri = format!("sqlite:///{}", path.display());
        Self::DatabaseConnection(format!(
            "Nie udało się połączyć z bazą danych {sqlite_uri}. Spróbuj jeszcze raz."
        ))
    }
}

fn ambiguous_noun(matches_count: usize) -> &'static str {
    if matches_count < 5 { "pasujące wyniki" } else { "pasujących wyników" }
}
