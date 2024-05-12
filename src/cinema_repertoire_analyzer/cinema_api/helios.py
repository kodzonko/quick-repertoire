from pydantic_core import Url

from cinema_repertoire_analyzer.cinema_api.cinema import Cinema
from cinema_repertoire_analyzer.cinema_api.models import Repertoire
from cinema_repertoire_analyzer.database.models import CinemaVenuesBase, HeliosVenues
from cinema_repertoire_analyzer.enums import CinemaChain


class Helios(Cinema):
    """Class handling interactions with www.helios.pl website."""

    def __init__(self, repertoire_url: Url, cinema_venues_url: Url) -> None:
        self.cinema_chain = CinemaChain.HELIOS
        self.repertoire_url = repertoire_url
        self.cinema_venues_url = cinema_venues_url

    def fetch_repertoire(self, date: str, venue_data: HeliosVenues) -> list[Repertoire]:
        """Download repertoire for a specified date and venue from the cinema website."""

    def fetch_cinema_venues_list(self) -> list[CinemaVenuesBase]:
        """Download list of cinema venues from the cinema website."""
