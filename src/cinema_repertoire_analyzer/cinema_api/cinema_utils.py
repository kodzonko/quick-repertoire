from cinema_repertoire_analyzer.cinema_api.cinema import Cinema
from cinema_repertoire_analyzer.cinema_api.cinema_city import CinemaCity
from cinema_repertoire_analyzer.cinema_api.helios import Helios
from cinema_repertoire_analyzer.cinema_api.multikino import Multikino
from cinema_repertoire_analyzer.enums import CinemaChain
from cinema_repertoire_analyzer.settings import CinemaSettings, Settings


def get_cinema_class_by_cinema_chain(cinema_chain: CinemaChain) -> type[Cinema]:
    """Get the cinema class for the given cinema chain."""
    cinema_name_to_cinema_class_mapping = {
        CinemaChain.CINEMA_CITY: CinemaCity,
        CinemaChain.HELIOS: Helios,
        CinemaChain.MULTIKINO: Multikino,
    }
    return cinema_name_to_cinema_class_mapping[cinema_chain]


def get_cinema_settings_by_cinema_chain(
    cinema_chain: CinemaChain, settings: Settings
) -> CinemaSettings:
    """Get the cinema class for the given cinema chain."""
    cinema_class_to_config_mapping = {
        CinemaChain.CINEMA_CITY: settings.cinema_city_settings,
        CinemaChain.HELIOS: settings.helios_settings,
        CinemaChain.MULTIKINO: settings.multikino_settings,
    }
    return cinema_class_to_config_mapping[cinema_chain]


def cinema_factory(cinema_chain: CinemaChain, settings: Settings) -> Cinema:
    """Factory function for creating cinema objects."""
    cinema_class: type[Cinema] = get_cinema_class_by_cinema_chain(cinema_chain)
    cinema_settings = get_cinema_settings_by_cinema_chain(cinema_chain, settings)

    return cinema_class(
        repertoire_url=cinema_settings.repertoire_url,
        cinema_venues_url=cinema_settings.venues_list_url,
    )
