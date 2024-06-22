import os
from pathlib import Path
from unittest.mock import patch

import pytest
from mockito import mock, when

from cinema_repertoire_analyzer.settings import Settings, get_settings


@pytest.fixture
def ENV_PATH() -> None:  # type: ignore[misc] # noqa: N802
    original_value = os.environ.get("ENV_PATH")
    os.environ["ENV_PATH"] = "/foo/bar/path/setting_file.env"
    yield
    if original_value is not None:
        os.environ["ENV_PATH"] = original_value
    else:
        os.unsetenv("ENV_PATH")


@pytest.fixture
def settings() -> Settings:
    return mock(Settings)  # type: ignore[no-any-return]


@pytest.fixture
def clear_cache() -> None:
    get_settings.cache_clear()


@pytest.mark.unit
@patch("cinema_repertoire_analyzer.settings.Settings")
def test_get_settings_returns_correct_settings_from_ENV_PATH_file(  # noqa: N802
    settings_patched,
    ENV_PATH: None,  # noqa: N803
    clear_cache: None,
) -> None:
    settings_patched.return_value = "settings_instance_from_file_under_ENV_PATH"
    when(Path).exists().thenReturn(True)

    assert get_settings() == "settings_instance_from_file_under_ENV_PATH"


@pytest.mark.unit
@patch("cinema_repertoire_analyzer.settings.Settings")
def test_get_settings_returns_correct_settings_from_default_env_file(
    mock_settings, clear_cache: None
) -> None:
    when(os.environ).get("ENV_PATH").thenReturn(None)
    when(Path).exists().thenReturn(True)
    mock_settings.return_value = "settings_instance_from_default_env_file"

    assert get_settings() == "settings_instance_from_default_env_file"


@pytest.mark.unit
@patch("cinema_repertoire_analyzer.settings.Settings")
def test_get_settings_returns_correct_settings_env_vars(mock_settings, clear_cache: None) -> None:
    when(os.environ).get("ENV_PATH").thenReturn(None)
    when(Path).exists().thenReturn(False)
    mock_settings.return_value = "settings_instance_from_env_vars"

    assert get_settings() == "settings_instance_from_env_vars"
