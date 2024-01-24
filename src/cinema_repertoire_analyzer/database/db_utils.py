from typing import Type

from cinema_repertoire_analyzer.database.models import (
    CinemaVenuesBase,
    CinemaCityVenues,
    MultikinoVenues,
    HeliosVenues,
)
from cinema_repertoire_analyzer.enums import CinemaChain


def get_table_by_cinema_chain(cinema_chain: CinemaChain) -> Type[CinemaVenuesBase]:
    """Get the table class for the given cinema chain."""
    cinema_chain_to_model_mapping = {
        CinemaChain.CINEMA_CITY: CinemaCityVenues,
        CinemaChain.MULTIKINO: MultikinoVenues,
        CinemaChain.HELIOS: HeliosVenues,
    }
    return cinema_chain_to_model_mapping[cinema_chain]
