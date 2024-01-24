from datetime import datetime

from cinema_repertoire_analyzer.cinema_api.cinema import Cinema
from cinema_repertoire_analyzer.cinema_api.models import Repertoire
from cinema_repertoire_analyzer.database.models import CinemaVenuesBase
from cinema_repertoire_analyzer.enums import CinemaChain


class Helios(Cinema):
    """Class handling interactions with www.helios.pl website."""

    def __init__(self, repertoire_url: str, cinema_venues_url: str) -> None:
        self.cinema_chain = CinemaChain.HELIOS
        self.repertoire_url = repertoire_url
        self.cinema_venues_url = cinema_venues_url

    def fetch_repertoire(self, date: datetime, cinema_venue: str) -> list[Repertoire]:
        pass

    def fetch_cinema_venues_list(self) -> list[CinemaVenuesBase]:
        pass
