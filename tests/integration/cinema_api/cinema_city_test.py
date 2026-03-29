import pytest

from cinema_repertoire_analyzer.cinema_api.cinema_city import CinemaCity
from cinema_repertoire_analyzer.cinema_api.models import CinemaVenue, MoviePlayDetails, Repertoire
from conftest import RESOURCE_DIR

pytestmark = pytest.mark.anyio


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
def venue_data() -> CinemaVenue:
    return CinemaVenue(chain_id="cinema-city", venue_id="1080", venue_name="Lodz - Manufaktura")


@pytest.fixture
def rendered_repertoire_html() -> str:
    with open(RESOURCE_DIR / "cinema_city_example_repertoire.html", encoding="utf-8") as file:
        return file.read()


@pytest.fixture
def rendered_venues_html() -> str:
    return """
    <select>
      <option value="">Wybierz kino</option>
      <option value="1080" data-tokens="Lodz - Manufaktura">Lodz - Manufaktura</option>
      <option value="1097" data-tokens="Wroclaw - Wroclavia">Wroclaw - Wroclavia</option>
      <option value="invalid" data-tokens="Ignored">Ignored</option>
      <option value="9999" data-tokens="null">Ignored</option>
    </select>
    """


@pytest.mark.integration
async def test_fetch_repertoire_parses_saved_repertoire_snapshot(
    cinema: CinemaCity,
    venue_data: CinemaVenue,
    rendered_repertoire_html: str,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    async def fake_fetch_rendered_html(url: str, selector: str) -> str:
        return rendered_repertoire_html

    monkeypatch.setattr(cinema, "_fetch_rendered_html", fake_fetch_rendered_html)

    repertoire = await cinema.fetch_repertoire(date="2023-04-01", venue_data=venue_data)

    assert repertoire[0] == Repertoire(
        title="65",
        genres="N/A",
        play_length="N/A",
        original_language="EN",
        play_details=[
            MoviePlayDetails(format="2D", play_language="NAP: PL", play_times=["17:45", "19:50"])
        ],
    )


@pytest.mark.integration
async def test_fetch_venues_filters_out_invalid_venues(
    cinema: CinemaCity, rendered_venues_html: str, monkeypatch: pytest.MonkeyPatch
) -> None:
    async def fake_fetch_rendered_html(url: str, selector: str) -> str:
        return rendered_venues_html

    monkeypatch.setattr(cinema, "_fetch_rendered_html", fake_fetch_rendered_html)

    venues = await cinema.fetch_venues()

    assert [(venue.chain_id, venue.venue_name, venue.venue_id) for venue in venues] == [
        ("cinema-city", "Lodz - Manufaktura", "1080"),
        ("cinema-city", "Wroclaw - Wroclavia", "1097"),
    ]
