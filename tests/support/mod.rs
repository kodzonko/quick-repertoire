#![allow(dead_code)]

use std::collections::{HashMap, VecDeque};
use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use async_trait::async_trait;
use quick_repertoire::app::AppDependencies;
use quick_repertoire::cinema::browser::{BrowserEvaluation, HtmlRenderer, RenderedPage};
use quick_repertoire::cinema::registry::{CinemaChainClient, RegisteredCinemaChain, Registry};
use quick_repertoire::config::{
    AppPaths, DefaultVenues, FileSystemRuntimeWriteAccessProbe, PromptAdapter,
    RuntimeWriteAccessProbe, SelectionChoice, Settings, UserPreferences,
};
use quick_repertoire::domain::{
    CinemaChainId, CinemaVenue, Repertoire, TmdbLookupMovie, TmdbMovieDetails,
};
use quick_repertoire::error::{AppError, AppResult};
use quick_repertoire::tmdb::TmdbService;
use tokio::time::sleep;

pub fn settings() -> Settings {
    let mut default_venues = DefaultVenues::default();
    default_venues.set(CinemaChainId::CinemaCity, Some("Wroclaw - Wroclavia".to_string()));

    Settings {
        user_preferences: UserPreferences {
            default_chain: CinemaChainId::CinemaCity,
            default_day: "dziś".to_string(),
            tmdb_access_token: Some("1234".to_string()),
            default_venues,
        },
    }
}

pub struct FailingWriteAccessProbe {
    pub error_kind: io::ErrorKind,
    pub message: String,
}

impl RuntimeWriteAccessProbe for FailingWriteAccessProbe {
    fn verify_target_writable(&self, _target_path: &Path, _runtime_dir: &Path) -> io::Result<()> {
        Err(io::Error::new(self.error_kind, self.message.clone()))
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
pub struct AcceptDefaultsPrompt {
    selections: Arc<Mutex<VecDeque<String>>>,
    texts: Arc<Mutex<VecDeque<String>>>,
}

impl AcceptDefaultsPrompt {
    pub fn new(selections: Vec<String>, texts: Vec<String>) -> Self {
        Self {
            selections: Arc::new(Mutex::new(VecDeque::from(selections))),
            texts: Arc::new(Mutex::new(VecDeque::from(texts))),
        }
    }
}

impl PromptAdapter for AcceptDefaultsPrompt {
    fn select(
        &self,
        _message: &str,
        choices: &[SelectionChoice],
        default: Option<&str>,
    ) -> AppResult<String> {
        if let Some(selection) =
            self.selections.lock().expect("prompt selections lock poisoned").pop_front()
        {
            return Ok(selection);
        }

        if let Some(choice) =
            default.and_then(|default| choices.iter().find(|choice| choice.value == default))
        {
            return Ok(choice.value.clone());
        }

        choices.first().map(|choice| choice.value.clone()).ok_or(AppError::ConfigurationAborted)
    }

    fn text(&self, _message: &str, default: &str) -> AppResult<String> {
        Ok(self
            .texts
            .lock()
            .expect("prompt texts lock poisoned")
            .pop_front()
            .unwrap_or_else(|| default.to_string()))
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
        _movies: &[TmdbLookupMovie],
        _access_token: &str,
    ) -> AppResult<HashMap<String, TmdbMovieDetails>> {
        match &self.error {
            Some(error) => Err(error.clone()),
            None => Ok(self.result.clone()),
        }
    }
}

pub fn dependencies<P: PromptAdapter + 'static>(
    runtime_dir: &Path,
    prompt: P,
    cinema_client: FakeCinemaClient,
    tmdb_service: FakeTmdbService,
) -> AppDependencies {
    dependencies_with_chains(
        runtime_dir,
        prompt,
        vec![registered_chain(CinemaChainId::CinemaCity, "Cinema City", cinema_client)],
        tmdb_service,
    )
}

pub fn dependencies_with_chains<P: PromptAdapter + 'static>(
    runtime_dir: &Path,
    prompt: P,
    chains: Vec<RegisteredCinemaChain>,
    tmdb_service: FakeTmdbService,
) -> AppDependencies {
    AppDependencies {
        paths: AppPaths::for_runtime_dir(runtime_dir.to_path_buf()),
        prompt: Box::new(prompt),
        registry: Registry::from_chains(chains),
        tmdb_client: Arc::new(tmdb_service),
        runtime_write_access_probe: Box::new(FileSystemRuntimeWriteAccessProbe),
    }
}

pub fn registered_chain(
    chain_id: CinemaChainId,
    display_name: &str,
    cinema_client: FakeCinemaClient,
) -> RegisteredCinemaChain {
    RegisteredCinemaChain {
        chain_id,
        display_name: display_name.to_string(),
        client: Arc::new(cinema_client),
    }
}

#[derive(Default)]
pub struct ConcurrencyTracker {
    in_flight: AtomicUsize,
    max_in_flight: AtomicUsize,
}

impl ConcurrencyTracker {
    pub fn max_in_flight(&self) -> usize {
        self.max_in_flight.load(Ordering::SeqCst)
    }

    fn enter(&self) -> ConcurrencyTrackerGuard<'_> {
        let current = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_in_flight.fetch_max(current, Ordering::SeqCst);
        ConcurrencyTrackerGuard { tracker: self }
    }
}

struct ConcurrencyTrackerGuard<'a> {
    tracker: &'a ConcurrencyTracker,
}

impl Drop for ConcurrencyTrackerGuard<'_> {
    fn drop(&mut self) {
        self.tracker.in_flight.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Clone)]
pub struct DelayedCinemaClient {
    pub venues: Vec<CinemaVenue>,
    pub delay: Duration,
    pub tracker: Arc<ConcurrencyTracker>,
}

#[async_trait]
impl CinemaChainClient for DelayedCinemaClient {
    async fn fetch_repertoire(
        &self,
        _date: &str,
        _venue: &CinemaVenue,
    ) -> AppResult<Vec<Repertoire>> {
        Ok(Vec::new())
    }

    async fn fetch_venues(&self) -> AppResult<Vec<CinemaVenue>> {
        let _guard = self.tracker.enter();
        sleep(self.delay).await;
        Ok(self.venues.clone())
    }
}

pub fn delayed_registered_chain(
    chain_id: CinemaChainId,
    display_name: &str,
    cinema_client: DelayedCinemaClient,
) -> RegisteredCinemaChain {
    RegisteredCinemaChain {
        chain_id,
        display_name: display_name.to_string(),
        client: Arc::new(cinema_client),
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

#[derive(Clone)]
pub struct FakeRenderedPageRenderer {
    pub html: String,
    pub evaluations: HashMap<String, String>,
}

#[async_trait]
impl HtmlRenderer for FakeRenderedPageRenderer {
    async fn render(&self, _url: &str, _wait_selector: &str) -> AppResult<String> {
        Ok(self.html.clone())
    }

    async fn render_with_evaluations(
        &self,
        _url: &str,
        _wait_selector: &str,
        evaluations: &[BrowserEvaluation],
    ) -> AppResult<RenderedPage> {
        let mut values = HashMap::new();
        for evaluation in evaluations {
            let value = self.evaluations.get(&evaluation.name).cloned().ok_or_else(|| {
                AppError::BrowserUnavailable(format!(
                    "Missing fake evaluation value for `{}`.",
                    evaluation.name
                ))
            })?;
            values.insert(evaluation.name.clone(), value);
        }

        Ok(RenderedPage { html: self.html.clone(), evaluations: values })
    }
}

pub fn json_response(
    status_code: u16,
    reason_phrase: &str,
    body: serde_json::Value,
    extra_headers: &[(&str, &str)],
) -> String {
    let body = body.to_string();
    let mut response = format!(
        "HTTP/1.1 {status_code} {reason_phrase}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );

    for (name, value) in extra_headers {
        response.push_str(&format!("{name}: {value}\r\n"));
    }

    response.push_str("\r\n");
    response.push_str(&body);
    response
}

pub fn start_sequenced_http_server(
    responses: Vec<String>,
) -> (String, Arc<Mutex<Vec<String>>>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
    let address = listener.local_addr().expect("test server should expose its address");
    let recorded_requests = Arc::new(Mutex::new(Vec::new()));
    let recorded_requests_handle = recorded_requests.clone();

    let server = thread::spawn(move || {
        for response in responses {
            let (mut stream, _) = listener.accept().expect("test server should accept a request");
            let mut request = Vec::new();
            let mut buffer = [0_u8; 1024];

            loop {
                let bytes_read = stream.read(&mut buffer).expect("request should be readable");
                if bytes_read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..bytes_read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }

            recorded_requests_handle
                .lock()
                .expect("recorded requests lock poisoned")
                .push(String::from_utf8(request).expect("request should be valid utf-8"));
            stream.write_all(response.as_bytes()).expect("response should be writable");
            stream.flush().expect("response should flush");
        }
    });

    (format!("http://{address}"), recorded_requests, server)
}
