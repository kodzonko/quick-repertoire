import pytest

from cinema_repertoire_analyzer.cinema_api.models import CinemaChainId
from cinema_repertoire_analyzer.cinema_api.registry import get_registered_chain
from cinema_repertoire_analyzer.exceptions import UnsupportedCinemaChainError
from cinema_repertoire_analyzer.settings import Settings


@pytest.mark.unit
def test_cinema_chain_id_from_value_parses_supported_chain() -> None:
    assert CinemaChainId.from_value("cinema-city") == CinemaChainId.CINEMA_CITY


@pytest.mark.unit
def test_cinema_chain_id_from_value_raises_for_unsupported_chain() -> None:
    with pytest.raises(UnsupportedCinemaChainError):
        CinemaChainId.from_value("helios")


@pytest.mark.unit
def test_registered_chain_returns_chain_specific_default_venue(settings: Settings) -> None:
    registered_chain = get_registered_chain(CinemaChainId.CINEMA_CITY)

    assert registered_chain.display_name == "Cinema City"
    assert registered_chain.default_venue_getter(settings) == "Wroclaw - Wroclavia"
