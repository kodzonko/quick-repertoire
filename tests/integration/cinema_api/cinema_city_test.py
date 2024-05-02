from datetime import datetime

import pytest

from cinema_repertoire_analyzer.cinema_api.cinema_city import CinemaCity
from cinema_repertoire_analyzer.cinema_api.models import Repertoire


@pytest.fixture
def cinema() -> CinemaCity:
    return CinemaCity(
        repertoire_url="https://www.cinema-city.pl/#/buy-tickets-by-cinema?in-cinema={cinema_venue_id}&at={repertoire_date}",
        cinema_venues_url="https://www.cinema-city.pl/#/buy-tickets-by-cinema",
    )


@pytest.mark.integration
def test_fetch_repertoire_downloads_and_parses_cinema_city_repertoire_correctly(
    cinema: CinemaCity,
) -> None:
    repertoire = cinema.fetch_repertoire(date=datetime.now(), venue_id=1097)
    assert len(repertoire) > 0
    assert isinstance(repertoire[0], Repertoire)


@pytest.mark.integration
def test_fetch_cinema_venues_list_downloads_list_of_cinema_venues_correctly() -> None: ...
