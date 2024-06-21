import os
from pathlib import Path

import pytest

from cinema_repertoire_analyzer.settings import get_settings, Settings

RESOURCE_DIR = Path(__file__).parent / "resources"
os.environ["ENV_PATH"] = str(RESOURCE_DIR / "test.env")


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
    if not (ENV_PATH := os.environ.get("ENV_PATH")) or not ENV_PATH.endswith("test.env"):
        raise ValueError("Env_PATH environment variable is not set or is not set to test.env file.")
    return get_settings()
