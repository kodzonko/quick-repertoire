from typing import Type

from loguru import logger

from cinema_repertoire_analyzer.cinema_api.cinema import Cinema
from cinema_repertoire_analyzer.cinema_api.cinema_city import CinemaCity
from cinema_repertoire_analyzer.cinema_api.helios import Helios
from cinema_repertoire_analyzer.cinema_api.multikino import Multikino
from cinema_repertoire_analyzer.database.models import (
    CinemaCityVenues,
    CinemaVenuesBase,
    HeliosVenues,
    MultikinoVenues,
)
from cinema_repertoire_analyzer.enums import CinemaChain
from cinema_repertoire_analyzer.exceptions import SettingsLoadError
from cinema_repertoire_analyzer.settings import CinemaSettings, Settings


def fill_string_template(text: str, **kwargs) -> str:
    """
    Verify that all variables in string are matched in the variables dictionary.

    Args:
        text: A string to parse.
        kwargs: Variables to format the string.

    Returns:
        True if all variables are present, False otherwise.

    Raises:
        SettingsLoadError: If some variables are missing.
    """
    try:
        return text.format(**kwargs)
    except IndexError:  # means no placeholders to substitute
        logger.info(
            "No placeholders to substitute in the url template. Returning unchanged."
        )
        return text
    except KeyError as e:  # means some variables are missing
        raise SettingsLoadError(
            "Unable to fill url template to make a request. Missing variable: %s." % e
        )


def get_table_by_cinema_chain(cinema_chain: CinemaChain) -> Type[CinemaVenuesBase]:
    """Get the table class for the given cinema chain."""
    cinema_chain_to_model_mapping = {
        CinemaChain.CINEMA_CITY: CinemaCityVenues,
        CinemaChain.MULTIKINO: MultikinoVenues,
        CinemaChain.HELIOS: HeliosVenues,
    }
    return cinema_chain_to_model_mapping[cinema_chain]


def get_cinema_class_by_cinema_chain(cinema_chain: CinemaChain) -> Type[Cinema]:
    """Get the cinema class for the given cinema chain."""
    cinema_name_to_cinema_class_mapping = {
        CinemaChain.CINEMA_CITY: CinemaCity,
        CinemaChain.HELIOS: Helios,
        CinemaChain.MULTIKINO: Multikino,
    }
    return cinema_name_to_cinema_class_mapping[cinema_chain]


def get_cinema_settings_by_cinema_chain(
    cinema_chain: CinemaChain,
    settings: Settings,
) -> CinemaSettings:
    """Get the cinema class for the given cinema chain."""
    cinema_class_to_config_mapping = {
        CinemaChain.CINEMA_CITY: settings.cinema_city_settings,
        CinemaChain.HELIOS: settings.helios_settings,
        CinemaChain.MULTIKINO: settings.multikino_settings,
    }
    return cinema_class_to_config_mapping[cinema_chain]


def cinema_factory(cinema_chain: CinemaChain, settings: Settings) -> Cinema:
    cinema_class = get_cinema_class_by_cinema_chain(cinema_chain)
    cinema_settings = get_cinema_settings_by_cinema_chain(cinema_chain, settings)

    return cinema_class(
        repertoire_url=cinema_settings.repertoire_url,
        cinema_venues_url=cinema_settings.venues_list_url,
    )
