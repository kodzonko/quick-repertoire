use std::collections::HashMap;
use std::time::Duration;

use futures::stream::{FuturesUnordered, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::cinema::registry::RegisteredCinemaChain;
use crate::config::Settings;
use crate::domain::{CinemaChainId, CinemaVenue};
use crate::error::{AppError, AppResult};

pub async fn fetch_registered_venues(
    settings: &Settings,
    chains: Vec<RegisteredCinemaChain>,
) -> AppResult<HashMap<CinemaChainId, Vec<CinemaVenue>>> {
    if chains.is_empty() {
        return Err(AppError::configuration("Brak zarejestrowanych sieci kin do skonfigurowania."));
    }

    let progress = VenueRefreshProgress::new(chains.len());
    let mut fetches = FuturesUnordered::new();

    for chain in chains {
        let progress_bar = progress.add_chain(&chain.display_name);
        let client = (chain.client_factory)(settings);
        let chain_id = chain.chain_id;
        let chain_display_name = chain.display_name.clone();

        fetches.push(async move {
            progress_bar.enable_steady_tick(Duration::from_millis(100));
            progress_bar.set_message("pobieranie listy lokali");
            let result = client.fetch_venues().await;
            progress_bar.disable_steady_tick();
            (chain_id, chain_display_name, progress_bar, result)
        });
    }

    let mut venues_by_chain = HashMap::new();
    let mut failed_chains = Vec::new();

    while let Some((chain_id, chain_display_name, progress_bar, result)) = fetches.next().await {
        match result {
            Ok(mut venues) if !venues.is_empty() => {
                venues.sort_by(|left, right| {
                    left.venue_name.to_lowercase().cmp(&right.venue_name.to_lowercase())
                });
                progress_bar.finish_with_message(format!("{} lokali", venues.len()));
                venues_by_chain.insert(chain_id, venues);
            }
            Ok(_) => {
                progress_bar.finish_with_message("brak lokali");
                failed_chains.push(chain_display_name.clone());
            }
            Err(_) => {
                progress_bar.finish_with_message("błąd");
                failed_chains.push(chain_display_name.clone());
            }
        }

        progress.advance(&chain_display_name);
    }

    progress.finish();

    if !failed_chains.is_empty() {
        failed_chains.sort();
        return Err(AppError::configuration(format!(
            "Nie udało się pobrać list lokali dla wszystkich sieci. Niepowodzenie: {}.",
            failed_chains.join(", ")
        )));
    }

    Ok(venues_by_chain)
}

struct VenueRefreshProgress {
    multi_progress: MultiProgress,
    overall: ProgressBar,
    chain_style: ProgressStyle,
}

impl VenueRefreshProgress {
    fn new(total_chains: usize) -> Self {
        let multi_progress = MultiProgress::new();
        let overall = multi_progress.add(ProgressBar::new(total_chains as u64));
        overall.set_style(
            ProgressStyle::with_template(
                "{bar:30.cyan/blue} {pos}/{len} obsłużonych sieci kin ({elapsed_precise})",
            )
            .expect("overall progress template must be valid")
            .progress_chars("=>-"),
        );
        overall.set_message("start");

        Self {
            multi_progress,
            overall,
            chain_style: ProgressStyle::with_template("{spinner:.green} {prefix:.bold} {msg}")
                .expect("chain progress template must be valid"),
        }
    }

    fn add_chain(&self, display_name: &str) -> ProgressBar {
        let progress_bar = self.multi_progress.add(ProgressBar::new_spinner());
        progress_bar.set_style(self.chain_style.clone());
        progress_bar.set_prefix(display_name.to_string());
        progress_bar
    }

    fn advance(&self, display_name: &str) {
        self.overall.inc(1);
        self.overall.set_message(display_name.to_string());
    }

    fn finish(&self) {
        self.overall.finish_with_message("Pobieranie list lokali zakończone");
    }
}
