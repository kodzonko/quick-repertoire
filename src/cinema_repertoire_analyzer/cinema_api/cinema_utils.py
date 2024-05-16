from cinema_repertoire_analyzer.cinema_api.cinema import Cinema
from cinema_repertoire_analyzer.cinema_api.cinema_city import CinemaCity
from cinema_repertoire_analyzer.cinema_api.helios import Helios
from cinema_repertoire_analyzer.cinema_api.multikino import Multikino
from cinema_repertoire_analyzer.database.models import VenueData
from cinema_repertoire_analyzer.enums import CinemaChain
from cinema_repertoire_analyzer.settings import CinemaSettings, Settings


def _get_cinema_class_by_cinema_chain(cinema_chain: CinemaChain) -> type[Cinema]:
    """Get the cinema class for the given cinema chain."""
    cinema_name_to_cinema_class_mapping = {
        CinemaChain.CINEMA_CITY: CinemaCity,
        CinemaChain.HELIOS: Helios,
        CinemaChain.MULTIKINO: Multikino,
    }
    return cinema_name_to_cinema_class_mapping[cinema_chain]


def _get_cinema_settings_by_cinema_chain(
    cinema_chain: CinemaChain, settings: Settings
) -> CinemaSettings:
    """Get the cinema class for the given cinema chain."""
    cinema_class_to_config_mapping = {
        CinemaChain.CINEMA_CITY: settings.CINEMA_CITY_SETTINGS,
        CinemaChain.HELIOS: settings.HELIOS_SETTINGS,
        CinemaChain.MULTIKINO: settings.MULTIKINO_SETTINGS,
    }
    return cinema_class_to_config_mapping[cinema_chain]


def _get_venue_data_for_specific_cinema_chain(
    cinema_chain: CinemaChain, venue_data: VenueData
) -> str:
    """Depending on the cinema chain, return either venue id or venue name needed for the
    repertoire fetcher.
    """  # noqa: D205
    if cinema_chain == CinemaChain.MULTIKINO:
        return venue_data.venue_name
    return venue_data.venue_id


def cinema_factory(cinema_chain: CinemaChain, settings: Settings) -> Cinema:
    """Factory function for creating cinema objects."""
    cinema_class: type[Cinema] = _get_cinema_class_by_cinema_chain(cinema_chain)
    cinema_settings = _get_cinema_settings_by_cinema_chain(cinema_chain, settings)

    return cinema_class(
        repertoire_url=cinema_settings.REPERTOIRE_URL,
        cinema_venues_url=cinema_settings.VENUES_LIST_URL,
    )
