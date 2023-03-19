from pathlib import Path

import toml
from loguru import logger

from exceptions import SettingsLoadError

SETTINGS_PATH = Path(__file__).parent.parent / "config.toml"

try:
    _loaded_settings = toml.load(SETTINGS_PATH)
    DEFAULT_CINEMA = _loaded_settings["user_preferences"]["default_cinema"]
    DEFAULT_CINEMA_VENUE = _loaded_settings["user_preferences"]["default_cinema_venue"]
    DEFAULT_DAY = _loaded_settings["user_preferences"]["default_day"]
    DB_FILE_PATH = _loaded_settings["db"]["db_file_path"]
except OSError as e:
    logger.error(
        "Unable to load settings file: %(file)s because of error: %(error)s",
        {"file": str(SETTINGS_PATH), "error": e},
    )
    raise SettingsLoadError(
        "Failed to load settings. Adjust path: %s or check permissions."
        % str(SETTINGS_PATH)
    )
except KeyError as e:
    logger.error(
        "Settings file incomplete: %(file)s. Missing value for: %(error)s",
        {"file": str(SETTINGS_PATH), "error": e},
    )
    raise SettingsLoadError("Settings file doesn't contain value for: %s." % e)
