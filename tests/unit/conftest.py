from pathlib import Path
from typing import Any

import pytest
from vcr import VCR

from conftest import RESOURCE_DIR


@pytest.fixture(autouse=True)
def unstub() -> None:
    from mockito import unstub

    yield
    unstub()


@pytest.fixture(scope="module")
def resources_dir() -> Path:
    return Path(__file__).parent.parent / "resources"


@pytest.fixture(scope="module")
def vcr_config() -> dict[str, Any]:
    return {
        "record_mode": "once",
        "match_on": ["method", "scheme", "host", "path", "query"],
        "cassette_library_dir": str(RESOURCE_DIR / "cassettes"),
        "path_transformer": VCR.ensure_suffix(".yaml"),
        "decode_compressed_response": True,
    }
