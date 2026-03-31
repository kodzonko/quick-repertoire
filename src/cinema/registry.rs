use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::cinema::cinema_city::{CinemaCity, HtmlRenderer};
use crate::config::{
    DEFAULT_CINEMA_CITY_REPERTOIRE_URL, DEFAULT_CINEMA_CITY_VENUES_LIST_URL, Settings,
};
use crate::domain::{CinemaChainId, CinemaVenue, Repertoire};
use crate::error::{AppError, AppResult};

#[async_trait]
pub trait CinemaChainClient: Send + Sync {
    async fn fetch_repertoire(&self, date: &str, venue: &CinemaVenue)
    -> AppResult<Vec<Repertoire>>;
    async fn fetch_venues(&self) -> AppResult<Vec<CinemaVenue>>;
}

pub type CinemaClientFactory = Arc<dyn Fn(&Settings) -> Box<dyn CinemaChainClient> + Send + Sync>;

#[derive(Clone)]
pub struct RegisteredCinemaChain {
    pub chain_id: CinemaChainId,
    pub display_name: String,
    pub client_factory: CinemaClientFactory,
}

#[derive(Clone)]
pub struct Registry {
    chains: HashMap<CinemaChainId, RegisteredCinemaChain>,
}

impl Registry {
    pub fn new(renderer: Arc<dyn HtmlRenderer>) -> Self {
        let cinema_city_renderer = renderer.clone();
        let cinema_city_factory = Arc::new(move |_settings: &Settings| {
            Box::new(CinemaCity::new(
                DEFAULT_CINEMA_CITY_REPERTOIRE_URL.to_string(),
                DEFAULT_CINEMA_CITY_VENUES_LIST_URL.to_string(),
                cinema_city_renderer.clone(),
            )) as Box<dyn CinemaChainClient>
        });

        Self::from_chains(vec![RegisteredCinemaChain {
            chain_id: CinemaChainId::CinemaCity,
            display_name: "Cinema City".to_string(),
            client_factory: cinema_city_factory,
        }])
    }

    pub fn from_chains(chains: Vec<RegisteredCinemaChain>) -> Self {
        Self { chains: chains.into_iter().map(|chain| (chain.chain_id, chain)).collect() }
    }

    pub fn get_registered_chain(
        &self,
        chain_id: CinemaChainId,
    ) -> AppResult<RegisteredCinemaChain> {
        self.chains.get(&chain_id).cloned().ok_or_else(|| AppError::UnsupportedCinemaChain {
            invalid_chain: chain_id.as_str().to_string(),
            supported_chains: CinemaChainId::supported_values().join(", "),
        })
    }

    pub fn get_registered_chains(&self) -> Vec<RegisteredCinemaChain> {
        let mut chains = self.chains.values().cloned().collect::<Vec<_>>();
        chains.sort_by(|left, right| left.display_name.cmp(&right.display_name));
        chains
    }
}
