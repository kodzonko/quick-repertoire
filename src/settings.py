from pathlib import Path
from typing import Any

import toml
from loguru import logger

from enums import Cinema
from exceptions import SettingsLoadError

PROJECT_ROOT = Path(__file__).parent.parent
SETTINGS_PATH = PROJECT_ROOT / "config.toml"


def load_settings() -> dict[str, Any]:
    """Load settings from the config file.

    Raises:
        SettingsLoadError: If settings file is missing or incorrect.
    """
    output: dict[str, Any] = {}
    try:
        _loaded_settings: dict[str, Any] = toml.load(SETTINGS_PATH)
        output["DEFAULT_CINEMA_VENUE"] = _loaded_settings["user_preferences"][
            "default_cinema_venue"
        ]

        if _loaded_settings["user_preferences"]["default_day"] not in [
            "today",
            "tomorrow",
        ]:
            raise SettingsLoadError(
                'DEFAULT_DAY value: "%s" in %s is invalid. Must be either '
                '"today" or "tomorrow".'
                % (
                    _loaded_settings["user_preferences"]["default_day"],
                    str(SETTINGS_PATH),
                )
            )
        else:
            output["DEFAULT_DAY"] = _loaded_settings["user_preferences"]["default_day"]
        if _loaded_settings["user_preferences"]["default_cinema"] not in [
            c.value for c in Cinema
        ]:
            raise SettingsLoadError(
                'DEFAULT_CINEMA value: "%(config_value)s" in %(config_path)s is '
                "invalid. Must be one of the following: %(value_valid_options)s."
                % {
                    "config_value": _loaded_settings["user_preferences"][
                        "default_cinema"
                    ],
                    "config_path": str(SETTINGS_PATH),
                    "value_valid_options": ", ".join([f'"{c.value}"' for c in Cinema]),
                }
            )
        else:
            output["DEFAULT_CINEMA"] = Cinema(
                _loaded_settings["user_preferences"]["default_cinema"]
            )
        if not Path(PROJECT_ROOT / _loaded_settings["db"]["db_file_path"]).exists():
            raise SettingsLoadError(
                'DB_FILE_PATH value: "%(config_value)s" in %(config_path)s is invalid.'
                " File doesn't exist."
                % {
                    "config_value": str(
                        PROJECT_ROOT / _loaded_settings["db"]["db_file_path"]
                    ),
                    "config_path": str(SETTINGS_PATH),
                }
            )
        else:
            output["DB_FILE_PATH"] = Path(_loaded_settings["db"]["db_file_path"])

        return output
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
        raise SettingsLoadError("Settings file doesn't contain value for: %s." % str(e))


_settings_dict = load_settings()

DEFAULT_CINEMA: Cinema = _settings_dict["DEFAULT_CINEMA"]
DEFAULT_CINEMA_VENUE: str = _settings_dict["DEFAULT_CINEMA_VENUE"]
DEFAULT_DAY: str = _settings_dict["DEFAULT_DAY"]
DB_FILE_PATH: Path = _settings_dict["DB_FILE_PATH"]
