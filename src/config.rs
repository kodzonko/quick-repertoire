use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use dialoguer::{Input, Select, theme::ColorfulTheme};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::cinema::registry::Registry;
use crate::domain::{CinemaChainId, CinemaVenue};
use crate::error::{AppError, AppResult};
use crate::persistence::DatabaseManager;

pub const CONFIG_FILE_NAME: &str = "config.ini";
pub const DB_FILE_NAME: &str = "db.sqlite";
pub const BINARY_NAME: &str = "quickrep";
pub const DEFAULT_DAY_CHOICES: [&str; 2] = ["dziś", "jutro"];
pub const DEFAULT_PRODUCTION_LOG_LEVEL: &str = "INFO";
pub const DEFAULT_DEVELOPMENT_LOG_LEVEL: &str = "DEBUG";
pub const HELP_AND_COMPLETION_FLAGS: [&str; 4] =
    ["-h", "--help", "--install-completion", "--show-completion"];
pub const DEFAULT_CINEMA_CITY_REPERTOIRE_URL: &str = "https://www.cinema-city.pl/kina/{cinema_venue_slug}/{cinema_venue_id}#/buy-tickets-by-cinema?in-cinema={cinema_venue_id}&at={repertoire_date}&view-mode=list";
pub const DEFAULT_CINEMA_CITY_VENUES_LIST_URL: &str =
    "https://www.cinema-city.pl/#/buy-tickets-by-cinema";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    runtime_dir: PathBuf,
}

impl AppPaths {
    pub fn from_current_exe() -> AppResult<Self> {
        let executable_path = std::env::current_exe().map_err(|error| {
            AppError::configuration(format!(
                "Nie udało się ustalić ścieżki binarki aplikacji: {error}"
            ))
        })?;
        let runtime_dir = executable_path.parent().ok_or_else(|| {
            AppError::configuration("Nie udało się ustalić katalogu binarki aplikacji.")
        })?;
        Ok(Self::for_runtime_dir(runtime_dir.to_path_buf()))
    }

    pub fn for_runtime_dir(runtime_dir: PathBuf) -> Self {
        Self { runtime_dir }
    }

    pub fn runtime_dir(&self) -> &Path {
        &self.runtime_dir
    }

    pub fn config_file(&self) -> PathBuf {
        self.runtime_dir.join(CONFIG_FILE_NAME)
    }

    pub fn db_file(&self) -> PathBuf {
        self.runtime_dir.join(DB_FILE_NAME)
    }
}

pub trait RuntimeWriteAccessProbe: Send + Sync {
    fn verify_target_writable(&self, target_path: &Path, runtime_dir: &Path) -> io::Result<()>;
}

pub struct FileSystemRuntimeWriteAccessProbe;

impl RuntimeWriteAccessProbe for FileSystemRuntimeWriteAccessProbe {
    fn verify_target_writable(&self, target_path: &Path, runtime_dir: &Path) -> io::Result<()> {
        verify_target_writable(target_path, runtime_dir)
    }
}

pub fn build_runtime_write_access_probe() -> Box<dyn RuntimeWriteAccessProbe> {
    Box::new(FileSystemRuntimeWriteAccessProbe)
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DefaultVenues {
    pub cinema_city: Option<String>,
}

impl DefaultVenues {
    pub fn get(&self, chain_id: CinemaChainId) -> Option<&str> {
        match chain_id {
            CinemaChainId::CinemaCity => self.cinema_city.as_deref(),
        }
    }

    pub fn set(&mut self, chain_id: CinemaChainId, value: Option<String>) {
        match chain_id {
            CinemaChainId::CinemaCity => self.cinema_city = value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserPreferences {
    pub default_chain: CinemaChainId,
    pub default_day: String,
    pub tmdb_access_token: Option<String>,
    pub default_venues: DefaultVenues,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            default_chain: CinemaChainId::CinemaCity,
            default_day: DEFAULT_DAY_CHOICES[0].to_string(),
            tmdb_access_token: None,
            default_venues: DefaultVenues::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Settings {
    pub user_preferences: UserPreferences,
}

impl Settings {
    pub fn get_default_venue(&self, chain_id: CinemaChainId) -> Option<&str> {
        self.user_preferences.default_venues.get(chain_id)
    }
}

pub fn default_log_level() -> &'static str {
    if cfg!(debug_assertions) {
        DEFAULT_DEVELOPMENT_LOG_LEVEL
    } else {
        DEFAULT_PRODUCTION_LOG_LEVEL
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionChoice {
    pub title: String,
    pub value: String,
}

pub trait PromptAdapter: Send + Sync {
    fn select(
        &self,
        message: &str,
        choices: &[SelectionChoice],
        default: Option<&str>,
    ) -> AppResult<String>;
    fn text(&self, message: &str, default: &str) -> AppResult<String>;
}

pub struct DialoguerPrompt;

impl PromptAdapter for DialoguerPrompt {
    fn select(
        &self,
        message: &str,
        choices: &[SelectionChoice],
        default: Option<&str>,
    ) -> AppResult<String> {
        let default_index = default
            .and_then(|value| choices.iter().position(|choice| choice.value == value))
            .unwrap_or(0);
        let titles = choices.iter().map(|choice| choice.title.as_str()).collect::<Vec<_>>();
        let result = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(message)
            .items(&titles)
            .default(default_index)
            .interact_opt()
            .map_err(map_prompt_error)?;
        result.map(|index| choices[index].value.clone()).ok_or(AppError::ConfigurationAborted)
    }

    fn text(&self, message: &str, default: &str) -> AppResult<String> {
        Input::<String>::with_theme(&ColorfulTheme::default())
            .with_prompt(message)
            .default(default.to_string())
            .allow_empty(true)
            .interact_text()
            .map_err(map_prompt_error)
    }
}

pub fn build_prompt_adapter() -> Box<dyn PromptAdapter> {
    Box::new(DialoguerPrompt)
}

pub fn should_skip_bootstrap_for_argv(argv: &[String]) -> bool {
    if argv.iter().any(|argument| HELP_AND_COMPLETION_FLAGS.contains(&argument.as_str())) {
        return true;
    }
    std::env::vars().any(|(key, _)| key.ends_with("_COMPLETE"))
}

pub fn should_defer_bootstrap_to_command(argv: &[String]) -> bool {
    argv.iter()
        .find(|argument| !argument.starts_with('-'))
        .is_some_and(|argument| argument == "configure")
}

pub async fn ensure_settings_for_argv(
    paths: &AppPaths,
    registry: &Registry,
    prompt: &dyn PromptAdapter,
) -> AppResult<Settings> {
    let write_access_probe = FileSystemRuntimeWriteAccessProbe;
    ensure_settings_for_argv_with_write_access_probe(paths, registry, prompt, &write_access_probe)
        .await
}

pub async fn ensure_settings_for_argv_with_write_access_probe(
    paths: &AppPaths,
    registry: &Registry,
    prompt: &dyn PromptAdapter,
    write_access_probe: &dyn RuntimeWriteAccessProbe,
) -> AppResult<Settings> {
    match load_settings(paths) {
        Ok(settings) => Ok(settings),
        Err(AppError::ConfigurationNotFound) => {
            run_interactive_configuration_with_write_access_probe(
                paths,
                None,
                registry,
                prompt,
                write_access_probe,
            )
            .await
        }
        Err(error) => Err(error),
    }
}

pub fn load_settings_if_available(paths: &AppPaths) -> Option<Settings> {
    load_settings(paths).ok()
}

pub fn load_settings(paths: &AppPaths) -> AppResult<Settings> {
    let config_path = paths.config_file();
    let content = fs::read_to_string(&config_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            AppError::ConfigurationNotFound
        } else {
            AppError::configuration("Nie udało się odczytać pliku config.ini.")
        }
    })?;

    let sections =
        parse_ini(&content).map_err(|_| AppError::configuration(configure_recovery_message()))?;

    Ok(Settings {
        user_preferences: UserPreferences {
            default_chain: CinemaChainId::from_value(
                get_required(&sections, "user_preferences", "default_chain")
                    .map_err(|_| AppError::configuration(configure_recovery_message()))?,
            )?,
            default_day: canonicalize_default_day(
                get_required(&sections, "user_preferences", "default_day")
                    .unwrap_or(DEFAULT_DAY_CHOICES[0]),
            ),
            tmdb_access_token: normalize_optional(
                get_optional(&sections, "user_preferences", "tmdb_access_token")
                    .unwrap_or_default()
                    .as_str(),
            ),
            default_venues: DefaultVenues {
                cinema_city: normalize_optional(
                    get_optional(&sections, "default_venues", "cinema_city")
                        .unwrap_or_default()
                        .as_str(),
                ),
            },
        },
    })
}

fn configure_recovery_message() -> String {
    format!(
        "Nie udało się wczytać config.ini. Uruchom `{BINARY_NAME} configure`, aby odtworzyć konfigurację."
    )
}

pub fn write_settings(settings: &Settings, paths: &AppPaths) -> AppResult<()> {
    let config_path = paths.config_file();
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|error| AppError::configuration(error.to_string()))?;
    }

    let config_body = format!(
        "[user_preferences]\n\
default_chain = {}\n\
default_day = {}\n\
tmdb_access_token = {}\n\
\n\
[default_venues]\n\
cinema_city = {}\n\
\n",
        settings.user_preferences.default_chain.as_str(),
        settings.user_preferences.default_day,
        settings.user_preferences.tmdb_access_token.clone().unwrap_or_default(),
        settings.user_preferences.default_venues.cinema_city.clone().unwrap_or_default(),
    );

    let temp_path = config_path.with_extension("tmp");
    fs::write(&temp_path, config_body)
        .map_err(|error| AppError::configuration(error.to_string()))?;
    fs::rename(&temp_path, &config_path)
        .map_err(|error| AppError::configuration(error.to_string()))?;
    Ok(())
}

pub async fn run_interactive_configuration(
    paths: &AppPaths,
    existing_settings: Option<Settings>,
    registry: &Registry,
    prompt: &dyn PromptAdapter,
) -> AppResult<Settings> {
    let write_access_probe = FileSystemRuntimeWriteAccessProbe;
    run_interactive_configuration_with_write_access_probe(
        paths,
        existing_settings,
        registry,
        prompt,
        &write_access_probe,
    )
    .await
}

pub async fn run_interactive_configuration_with_write_access_probe(
    paths: &AppPaths,
    existing_settings: Option<Settings>,
    registry: &Registry,
    prompt: &dyn PromptAdapter,
    write_access_probe: &dyn RuntimeWriteAccessProbe,
) -> AppResult<Settings> {
    verify_runtime_write_access(paths, write_access_probe)?;

    let mut base_settings = existing_settings.unwrap_or_default();
    base_settings.user_preferences.default_day =
        canonicalize_default_day(&base_settings.user_preferences.default_day);
    let mut working_settings = base_settings.clone();
    let venues_by_chain = fetch_all_registered_venues(&working_settings, registry).await?;

    let chain_choices = registry
        .get_registered_chains()
        .iter()
        .map(|chain| SelectionChoice {
            title: chain.display_name.clone(),
            value: chain.chain_id.as_str().to_string(),
        })
        .collect::<Vec<_>>();
    let selected_default_chain = CinemaChainId::from_value(&prompt.select(
        "Wybierz domyślną sieć kin:",
        &chain_choices,
        Some(base_settings.user_preferences.default_chain.as_str()),
    )?)?;
    let selected_default_day = prompt.select(
        "Wybierz domyślną datę repertuaru:",
        &DEFAULT_DAY_CHOICES
            .iter()
            .map(|choice| SelectionChoice {
                title: (*choice).to_string(),
                value: (*choice).to_string(),
            })
            .collect::<Vec<_>>(),
        Some(base_settings.user_preferences.default_day.as_str()),
    )?;
    let available_venues = venues_by_chain
        .get(&selected_default_chain)
        .ok_or_else(|| AppError::configuration("Brak lokali dla wybranej sieci."))?;
    let venue_choices = available_venues
        .iter()
        .map(|venue| SelectionChoice {
            title: venue.venue_name.clone(),
            value: venue.venue_name.clone(),
        })
        .collect::<Vec<_>>();
    let selected_default_venue = prompt.select(
        "Wybierz domyślny lokal:",
        &venue_choices,
        base_settings
            .get_default_venue(selected_default_chain)
            .or_else(|| available_venues.first().map(|venue| venue.venue_name.as_str())),
    )?;
    let selected_tmdb_access_token = prompt.text(
        "Podaj dane TMDB (API Read Access Token lub Klucz API; pozostaw puste, aby wyłączyć TMDB):",
        base_settings.user_preferences.tmdb_access_token.as_deref().unwrap_or(""),
    )?;

    working_settings.user_preferences.default_chain = selected_default_chain;
    working_settings.user_preferences.default_day = canonicalize_default_day(&selected_default_day);
    working_settings.user_preferences.tmdb_access_token =
        normalize_optional(&selected_tmdb_access_token);
    working_settings
        .user_preferences
        .default_venues
        .set(selected_default_chain, Some(selected_default_venue));

    persist_venues(paths, &venues_by_chain)?;
    write_settings(&working_settings, paths)?;
    Ok(working_settings)
}

async fn fetch_all_registered_venues(
    settings: &Settings,
    registry: &Registry,
) -> AppResult<HashMap<CinemaChainId, Vec<CinemaVenue>>> {
    let chains = registry.get_registered_chains();
    if chains.is_empty() {
        return Err(AppError::configuration("Brak zarejestrowanych sieci kin do skonfigurowania."));
    }

    let progress_style = ProgressStyle::with_template("{spinner:.green} {msg}")
        .map_err(|error| AppError::configuration(error.to_string()))?;
    let multi_progress = MultiProgress::new();
    let mut venues_by_chain = HashMap::new();
    let mut failed_chains = Vec::new();

    for chain in chains {
        let progress_bar = multi_progress.add(ProgressBar::new_spinner());
        progress_bar.set_style(progress_style.clone());
        progress_bar.set_message(format!("{}: pobieranie", chain.display_name));

        let client = (chain.client_factory)(settings);
        match client.fetch_venues().await {
            Ok(mut venues) if !venues.is_empty() => {
                venues.sort_by(|left, right| {
                    left.venue_name.to_lowercase().cmp(&right.venue_name.to_lowercase())
                });
                progress_bar.finish_with_message(format!(
                    "{}: {} lokali",
                    chain.display_name,
                    venues.len()
                ));
                venues_by_chain.insert(chain.chain_id, venues);
            }
            Ok(_) => {
                progress_bar.finish_with_message(format!("{}: brak lokali", chain.display_name));
                failed_chains.push(chain.display_name.clone());
            }
            Err(_) => {
                progress_bar.finish_with_message(format!("{}: błąd", chain.display_name));
                failed_chains.push(chain.display_name.clone());
            }
        }
    }

    if !failed_chains.is_empty() {
        failed_chains.sort();
        return Err(AppError::configuration(format!(
            "Nie udało się pobrać list lokali dla wszystkich sieci. Niepowodzenie: {}.",
            failed_chains.join(", ")
        )));
    }

    Ok(venues_by_chain)
}

fn persist_venues(
    paths: &AppPaths,
    venues_by_chain: &HashMap<CinemaChainId, Vec<CinemaVenue>>,
) -> AppResult<()> {
    let db_manager = DatabaseManager::new(paths.db_file())?;
    let payload = venues_by_chain
        .iter()
        .map(|(chain_id, venues)| (chain_id.as_str().to_string(), venues.clone()))
        .collect::<HashMap<_, _>>();
    db_manager.replace_venues_batch(&payload)
}

fn verify_runtime_write_access(
    paths: &AppPaths,
    write_access_probe: &dyn RuntimeWriteAccessProbe,
) -> AppResult<()> {
    for target_path in [paths.config_file(), paths.db_file()] {
        write_access_probe
            .verify_target_writable(&target_path, paths.runtime_dir())
            .map_err(|error| map_runtime_write_access_error(paths, &target_path, error))?;
    }
    Ok(())
}

fn verify_target_writable(target_path: &Path, runtime_dir: &Path) -> io::Result<()> {
    if target_path.exists() {
        OpenOptions::new().write(true).open(target_path).map(|_| ())
    } else {
        let probe_path = runtime_probe_path(target_path, runtime_dir);
        let probe_file = OpenOptions::new().write(true).create_new(true).open(&probe_path)?;
        drop(probe_file);
        fs::remove_file(&probe_path)
    }
}

fn runtime_probe_path(target_path: &Path, runtime_dir: &Path) -> PathBuf {
    let file_name =
        target_path.file_name().and_then(|value| value.to_str()).unwrap_or("runtime-file");
    runtime_dir.join(format!(".{file_name}.write-test.{}", std::process::id()))
}

fn map_runtime_write_access_error(
    paths: &AppPaths,
    target_path: &Path,
    error: io::Error,
) -> AppError {
    match error.kind() {
        io::ErrorKind::PermissionDenied => AppError::configuration(format!(
            "Brak uprawnień do zapisu w katalogu aplikacji {}. Nie można przygotować pliku {}. Uruchom aplikację z podwyższonymi uprawnieniami albo przenieś binarkę do katalogu, w którym masz prawo zapisu.",
            paths.runtime_dir().display(),
            target_path.display(),
        )),
        _ => AppError::configuration(format!(
            "Nie udało się sprawdzić możliwości zapisu w katalogu aplikacji {} dla pliku {}: {}",
            paths.runtime_dir().display(),
            target_path.display(),
            error
        )),
    }
}

fn parse_ini(content: &str) -> Result<HashMap<String, HashMap<String, String>>, ()> {
    let mut sections = HashMap::<String, HashMap<String, String>>::new();
    let mut current_section: Option<String> = None;

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            let section_name = line[1..line.len() - 1].trim().to_string();
            sections.entry(section_name.clone()).or_default();
            current_section = Some(section_name);
            continue;
        }

        let section = current_section.as_ref().ok_or(())?;
        let (key, value) = line.split_once('=').ok_or(())?;
        sections
            .entry(section.clone())
            .or_default()
            .insert(key.trim().to_string(), value.trim().to_string());
    }

    Ok(sections)
}

fn get_required<'a>(
    sections: &'a HashMap<String, HashMap<String, String>>,
    section: &str,
    key: &str,
) -> Result<&'a str, ()> {
    sections.get(section).and_then(|values| values.get(key)).map(String::as_str).ok_or(())
}

fn get_optional(
    sections: &HashMap<String, HashMap<String, String>>,
    section: &str,
    key: &str,
) -> Option<String> {
    sections.get(section).and_then(|values| values.get(key)).cloned()
}

fn normalize_optional(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
}

fn canonicalize_default_day(day: &str) -> String {
    let trimmed = day.trim();
    let normalized = trimmed.to_lowercase();
    match normalized.as_str() {
        "dziś" | "dzis" | "dzisiaj" | "today" => DEFAULT_DAY_CHOICES[0].to_string(),
        "jutro" | "tomorrow" => DEFAULT_DAY_CHOICES[1].to_string(),
        _ if trimmed.is_empty() => DEFAULT_DAY_CHOICES[0].to_string(),
        _ => trimmed.to_string(),
    }
}

fn map_prompt_error(error: dialoguer::Error) -> AppError {
    if error.to_string().to_ascii_lowercase().contains("interrupted") {
        AppError::ConfigurationAborted
    } else {
        AppError::configuration(error.to_string())
    }
}
