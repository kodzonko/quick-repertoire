import os
from pathlib import Path
from typing import Any

import pytest
import vcr

RESOURCE_DIR = Path(__file__).parent / "resources"
os.environ["ENV_PATH"] = str(RESOURCE_DIR / "test.env")


@pytest.fixture(scope="session")
def vcr_config() -> dict[str, Any]:
    return {
        "record_mode": "once",
        "match_on": ["method", "uri"],
        "cassette_library_dir": str(RESOURCE_DIR / "vcr_cassettes"),
        "path_transformer": vcr.VCR.ensure_suffix(".yaml"),
        "decode_compressed_response": True,
    }
