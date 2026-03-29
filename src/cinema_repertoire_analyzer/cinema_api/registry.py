from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass
from typing import Protocol, cast

from cinema_repertoire_analyzer.cinema_api.cinema_city import CinemaCity
from cinema_repertoire_analyzer.cinema_api.models import CinemaChainId, CinemaVenue, Repertoire
from cinema_repertoire_analyzer.exceptions import UnsupportedCinemaChainError
from cinema_repertoire_analyzer.settings import Settings


class CinemaChainClient(Protocol):
    """Interface implemented by chain-specific cinema clients."""

    async def fetch_repertoire(self, date: str, venue: CinemaVenue) -> list[Repertoire]:
        """Fetch repertoire for a selected venue and date."""

    async def fetch_venues(self) -> list[CinemaVenue]:
        """Fetch all venues available for the chain."""


@dataclass(frozen=True)
class RegisteredCinemaChain:
    """Registered cinema chain metadata and factory hooks."""

    chain_id: CinemaChainId
    display_name: str
    default_venue_getter: Callable[[Settings], str | None]
    client_factory: Callable[[Settings], CinemaChainClient]


def _build_cinema_city_client(settings: Settings) -> CinemaChainClient:
    cinema_city_settings = settings.CINEMA_CHAINS.CINEMA_CITY
    return cast(
        CinemaChainClient,
        CinemaCity(
            repertoire_url=cinema_city_settings.REPERTOIRE_URL,
            cinema_venues_url=cinema_city_settings.VENUES_LIST_URL,
        ),
    )


def _get_cinema_city_default_venue(settings: Settings) -> str | None:
    return settings.USER_PREFERENCES.DEFAULT_VENUES.CINEMA_CITY


REGISTERED_CINEMA_CHAINS: dict[CinemaChainId, RegisteredCinemaChain] = {
    CinemaChainId.CINEMA_CITY: RegisteredCinemaChain(
        chain_id=CinemaChainId.CINEMA_CITY,
        display_name="Cinema City",
        default_venue_getter=_get_cinema_city_default_venue,
        client_factory=_build_cinema_city_client,
    )
}


def get_registered_chain(chain_id: CinemaChainId) -> RegisteredCinemaChain:
    """Get metadata and factory hooks for a registered cinema chain."""
    try:
        return REGISTERED_CINEMA_CHAINS[chain_id]
    except KeyError as error:
        raise UnsupportedCinemaChainError(
            invalid_chain=chain_id.value,
            supported_chains=", ".join(chain.value for chain in REGISTERED_CINEMA_CHAINS),
        ) from error
