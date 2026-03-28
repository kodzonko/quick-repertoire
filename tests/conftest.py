import os
from pathlib import Path

import pytest

from cinema_repertoire_analyzer.settings import Settings, get_settings

RESOURCE_DIR = Path(__file__).parent / "resources"


@pytest.fixture(scope="session")
def vcr_config():
    return {
        "cassette_library_dir": str(RESOURCE_DIR / "vcr_cassettes"),
        "match_on": ["method", "uri", "path"],
        "filter_headers": [("authorization", "DUMMY")],
        "record_mode": "once",
    }


@pytest.fixture
def settings() -> Settings:
    os.environ.pop("ENV_PATH", None)
    os.environ["LOGURU_LEVEL"] = "TRACE"
    os.environ["DB_FILE"] = str(RESOURCE_DIR / "test_db.sqlite")
    os.environ["USER_PREFERENCES__DEFAULT_CINEMA_VENUE"] = "Wrocław - Wroclavia"
    os.environ["USER_PREFERENCES__DEFAULT_DAY"] = "today"
    os.environ["USER_PREFERENCES__TMDB_ACCESS_TOKEN"] = "1234"
    os.environ["CINEMA_CITY_SETTINGS__REPERTOIRE_URL"] = (
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema?"
        "in-cinema={cinema_venue_id}&at={repertoire_date}"
    )
    os.environ["CINEMA_CITY_SETTINGS__VENUES_LIST_URL"] = (
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema"
    )
    get_settings.cache_clear()
    return get_settings()
