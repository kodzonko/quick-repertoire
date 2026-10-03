use std::collections::HashMap;

use futures::stream::{FuturesUnordered, StreamExt};

use crate::cinema::registry::RegisteredCinemaChain;
use crate::domain::{CinemaChainId, CinemaVenue};
use crate::error::{AppError, AppResult};

pub async fn fetch_registered_venues(
    chains: Vec<RegisteredCinemaChain>,
) -> AppResult<HashMap<CinemaChainId, Vec<CinemaVenue>>> {
    if chains.is_empty() {
        return Err(AppError::configuration("Brak zarejestrowanych sieci kin do skonfigurowania."));
    }

    let mut fetches = FuturesUnordered::new();

    for chain in chains {
        let chain_id = chain.chain_id;
        let chain_display_name = chain.display_name;
        let client = chain.client;

        fetches.push(async move {
            let result = client.fetch_venues().await;
            (chain_id, chain_display_name, result)
        });
    }

    let mut venues_by_chain = HashMap::new();
    let mut failed_chains = Vec::new();

    while let Some((chain_id, chain_display_name, result)) = fetches.next().await {
        match result {
            Ok(mut venues) if !venues.is_empty() => {
                venues.sort_by(|left, right| {
                    left.venue_name.to_lowercase().cmp(&right.venue_name.to_lowercase())
                });
                eprintln!("{chain_display_name}: {} lokali", venues.len());
                venues_by_chain.insert(chain_id, venues);
            }
            Ok(_) => {
                eprintln!("{chain_display_name}: brak lokali");
                failed_chains.push(chain_display_name.clone());
            }
            Err(_) => {
                eprintln!("{chain_display_name}: błąd");
                failed_chains.push(chain_display_name.clone());
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
