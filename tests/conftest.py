from pathlib import Path

import pytest

from cinema_repertoire_analyzer.cinema_api.models import CinemaChainId
from cinema_repertoire_analyzer.settings import (
    CinemaChainSettings,
    CinemaChainsSettings,
    DefaultVenues,
    Settings,
    UserPreferences,
)

RESOURCE_DIR = Path(__file__).parent / "resources"


def remove_compression_headers(response: dict) -> dict:
    """Normalize recorded responses so playback matches the stored body."""
    headers = response.get("headers", {})
    headers.pop("Content-Encoding", None)
    headers.pop("content-encoding", None)
    return response


@pytest.fixture(scope="session")
def vcr_config():
    return {
        "cassette_library_dir": str(RESOURCE_DIR / "vcr_cassettes"),
        "match_on": ["method", "uri", "path"],
        "filter_headers": [("authorization", "DUMMY")],
        "decode_compressed_response": True,
        "before_record_response": remove_compression_headers,
        "record_mode": "once",
    }


@pytest.fixture
def anyio_backend() -> str:
    return "trio"


@pytest.fixture
def settings() -> Settings:
    return Settings(
        db_file=RESOURCE_DIR / "test_db.sqlite",
        loguru_level="TRACE",
        user_preferences=UserPreferences(
            default_chain=CinemaChainId.CINEMA_CITY,
            default_day="today",
            tmdb_access_token="1234",
            default_venues=DefaultVenues(cinema_city="Wroclaw - Wroclavia"),
        ),
        cinema_chains=CinemaChainsSettings(
            cinema_city=CinemaChainSettings(
                repertoire_url="https://www.cinema-city.pl/#/buy-tickets-by-cinema?"
                "in-cinema={cinema_venue_id}&at={repertoire_date}",
                venues_list_url="https://www.cinema-city.pl/#/buy-tickets-by-cinema",
            )
        ),
    )
