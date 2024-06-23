from datetime import datetime

import pytest

from cinema_repertoire_analyzer.cinema_api.cinema_city import CinemaCity
from cinema_repertoire_analyzer.cinema_api.models import Repertoire
from cinema_repertoire_analyzer.database.models import CinemaVenues


@pytest.fixture
def cinema() -> CinemaCity:
    return CinemaCity(
        repertoire_url=(
            "https://www.cinema-city.pl/#/buy-tickets-by-cinema?in-cinema="
            "{cinema_venue_id}&at={repertoire_date}"
        ),
        cinema_venues_url="https://www.cinema-city.pl/#/buy-tickets-by-cinema",
    )


@pytest.fixture
def venue_data() -> CinemaVenues:
    return CinemaVenues(venue_id="1080", venue_name="Łódź Manufaktura")


@pytest.mark.integration
def test_fetch_repertoire_downloads_and_parses_cinema_city_repertoire_correctly(
    cinema: CinemaCity, venue_data: CinemaVenues
) -> None:
    repertoire = cinema.fetch_repertoire(
        date=datetime.now().strftime("%Y-%m-%d"), venue_data=venue_data
    )
    assert len(repertoire) > 0
    assert isinstance(repertoire[0], Repertoire)


@pytest.mark.integration
def test_fetch_cinema_venues_list_downloads_list_of_cinema_venues_correctly() -> None: ...
