from pathlib import Path
from typing import Any

import toml
from loguru import logger

from cinema_api.models import CinemaConfig
from enums import CinemaChain
from exceptions import SettingsLoadError

PROJECT_ROOT = Path(__file__).parent.parent
SETTINGS_PATH = PROJECT_ROOT / "config.toml"


def load_settings(config_file: Path | str = SETTINGS_PATH) -> dict[str, Any]:
    """Load settings from the config file.

    Raises:
        SettingsLoadError: If settings file is missing or incorrect.
    """
    output: dict[str, Any] = {}
    try:
        _loaded_settings: dict[str, Any] = toml.load(config_file)
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
                    str(config_file),
                )
            )
        else:
            output["DEFAULT_DAY"] = _loaded_settings["user_preferences"]["default_day"]
        if _loaded_settings["user_preferences"]["default_cinema"] not in [
            c.value for c in CinemaChain
        ]:
            raise SettingsLoadError(
                'DEFAULT_CINEMA value: "%(config_value)s" in %(config_path)s is '
                "invalid. Must be one of the following: %(value_valid_options)s."
                % {
                    "config_value": _loaded_settings["user_preferences"][
                        "default_cinema"
                    ],
                    "config_path": str(config_file),
                    "value_valid_options": ", ".join(
                        [f'"{c.value}"' for c in CinemaChain]
                    ),
                }
            )
        else:
            output["DEFAULT_CINEMA"] = CinemaChain(
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
                    "config_path": str(config_file),
                }
            )
        else:
            output["DB_FILE_PATH"] = Path(_loaded_settings["db"]["db_file_path"])

        return output
    except OSError as e:
        logger.error(
            "Unable to load settings file: %(file)s because of error: %(error)s",
            {"file": str(config_file), "error": e},
        )
        raise SettingsLoadError(
            'Failed to load settings. Adjust path: "%s" or check permissions.'
            % str(config_file)
        )
    except KeyError as e:
        logger.error(
            'Settings file incomplete: "%(file)s". Missing value for: %(error)s',
            {"file": str(config_file), "error": e},
        )
        raise SettingsLoadError("Settings file doesn't contain value for: %s." % str(e))


_settings_dict = load_settings()

DEFAULT_CINEMA: CinemaChain = _settings_dict["DEFAULT_CINEMA"]
DEFAULT_CINEMA_VENUE: str = _settings_dict["DEFAULT_CINEMA_VENUE"]
DEFAULT_DAY: str = _settings_dict["DEFAULT_DAY"]
DB_FILE_PATH: Path = _settings_dict["DB_FILE_PATH"]


def load_config_for_cinema(
    cinema_chain: CinemaChain, config_file: Path | str = SETTINGS_PATH
) -> CinemaConfig:
    """Load configuration for a specific cinema from config file."""
    logger.info("Loading settings for cinema: %s." % cinema_chain.value)
    try:
        _loaded_settings: dict[str, Any] = toml.load(config_file)
        output: CinemaConfig = _loaded_settings["cinemas"][str(cinema_chain)]
        logger.info(
            "Settings for cinema: %s loaded correctly." % cinema_chain.value
        )
        return output
    except OSError as e:
        logger.error(
            "Unable to load settings file: %(file)s because of error: %(error)s",
            {"file": str(config_file), "error": e},
        )
        raise SettingsLoadError(
            'Failed to load settings. Adjust path: "%s" or check permissions.'
            % str(config_file)
        )
    except KeyError as e:
        logger.error(
            'Settings file incomplete: "%(file)s". Missing value for: %(error)s',
            {"file": str(config_file), "error": e},
        )
        raise SettingsLoadError("Settings file doesn't contain value for: %s." % str(e))
