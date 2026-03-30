#![allow(dead_code)]

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use quick_repertoire::app::AppDependencies;
use quick_repertoire::cinema::cinema_city::HtmlRenderer;
use quick_repertoire::cinema::registry::{CinemaChainClient, RegisteredCinemaChain, Registry};
use quick_repertoire::config::{
    CinemaChainSettings, CinemaChainsSettings, DefaultVenues, PromptAdapter, SelectionChoice,
    Settings, UserPreferences,
};
use quick_repertoire::domain::{CinemaChainId, CinemaVenue, Repertoire, TmdbMovieDetails};
use quick_repertoire::error::{AppError, AppResult};
use quick_repertoire::tmdb::TmdbService;

pub fn settings(project_root: &Path) -> Settings {
    Settings {
        project_root: project_root.to_path_buf(),
        db_file: project_root.join("test_db.sqlite"),
        user_preferences: UserPreferences {
            default_chain: CinemaChainId::CinemaCity,
            default_day: "today".to_string(),
            tmdb_access_token: Some("1234".to_string()),
            default_venues: DefaultVenues {
                cinema_city: Some("Wroclaw - Wroclavia".to_string()),
            },
        },
        cinema_chains: CinemaChainsSettings {
            cinema_city: CinemaChainSettings {
                repertoire_url: "https://www.cinema-city.pl/#/buy-tickets-by-cinema?in-cinema={cinema_venue_id}&at={repertoire_date}".to_string(),
                venues_list_url: "https://www.cinema-city.pl/#/buy-tickets-by-cinema".to_string(),
            },
        },
        loguru_level: "TRACE".to_string(),
    }
}

#[derive(Clone)]
pub struct FakePrompt {
    selections: Arc<Mutex<VecDeque<String>>>,
    texts: Arc<Mutex<VecDeque<String>>>,
}

impl FakePrompt {
    pub fn new(selections: Vec<String>, texts: Vec<String>) -> Self {
        Self {
            selections: Arc::new(Mutex::new(VecDeque::from(selections))),
            texts: Arc::new(Mutex::new(VecDeque::from(texts))),
        }
    }
}

impl PromptAdapter for FakePrompt {
    fn select(
        &self,
        _message: &str,
        _choices: &[SelectionChoice],
        _default: Option<&str>,
    ) -> AppResult<String> {
        self.selections
            .lock()
            .expect("prompt selections lock poisoned")
            .pop_front()
            .ok_or(AppError::ConfigurationAborted)
    }

    fn text(&self, _message: &str, _default: &str) -> AppResult<String> {
        self.texts
            .lock()
            .expect("prompt texts lock poisoned")
            .pop_front()
            .ok_or(AppError::ConfigurationAborted)
    }
}

#[derive(Clone)]
pub struct FakeCinemaClient {
    pub repertoire: Vec<Repertoire>,
    pub venues: Vec<CinemaVenue>,
    pub repertoire_error: Option<AppError>,
    pub venues_error: Option<AppError>,
}

impl FakeCinemaClient {
    pub fn new(repertoire: Vec<Repertoire>, venues: Vec<CinemaVenue>) -> Self {
        Self { repertoire, venues, repertoire_error: None, venues_error: None }
    }
}

#[async_trait]
impl CinemaChainClient for FakeCinemaClient {
    async fn fetch_repertoire(
        &self,
        _date: &str,
        _venue: &CinemaVenue,
    ) -> AppResult<Vec<Repertoire>> {
        match &self.repertoire_error {
            Some(error) => Err(error.clone()),
            None => Ok(self.repertoire.clone()),
        }
    }

    async fn fetch_venues(&self) -> AppResult<Vec<CinemaVenue>> {
        match &self.venues_error {
            Some(error) => Err(error.clone()),
            None => Ok(self.venues.clone()),
        }
    }
}

#[derive(Clone)]
pub struct FakeTmdbService {
    pub result: HashMap<String, TmdbMovieDetails>,
    pub error: Option<AppError>,
}

#[async_trait]
impl TmdbService for FakeTmdbService {
    async fn get_movie_ratings_and_summaries(
        &self,
        _movie_names: &[String],
        _access_token: &str,
    ) -> AppResult<HashMap<String, TmdbMovieDetails>> {
        match &self.error {
            Some(error) => Err(error.clone()),
            None => Ok(self.result.clone()),
        }
    }
}

pub fn dependencies(
    project_root: &Path,
    prompt: FakePrompt,
    cinema_client: FakeCinemaClient,
    tmdb_service: FakeTmdbService,
) -> AppDependencies {
    let factory_client = cinema_client.clone();
    let chain = RegisteredCinemaChain {
        chain_id: CinemaChainId::CinemaCity,
        display_name: "Cinema City".to_string(),
        client_factory: Arc::new(move |_| Box::new(factory_client.clone())),
    };
    AppDependencies {
        project_root: project_root.to_path_buf(),
        prompt: Box::new(prompt),
        registry: Registry::from_chains(vec![chain]),
        tmdb_client: Arc::new(tmdb_service),
    }
}

#[derive(Clone)]
pub struct FakeHtmlRenderer {
    pub html: String,
}

#[async_trait]
impl HtmlRenderer for FakeHtmlRenderer {
    async fn render(&self, _url: &str, _wait_selector: &str) -> AppResult<String> {
        Ok(self.html.clone())
    }
}
