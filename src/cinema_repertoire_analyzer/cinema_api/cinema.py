"""Module containing logic interacting with cinema websites.

Due to lack of official APIs for the supported cinemas, data is downloaded using web
scraping. This module is the de-facto simplified API for the supported cinemas.

Be mindful of the fact that the websites may change their structure at any time,
urls have been abstracted away to the config file, but the logic may need to be affected
as well.

Don't overuse these functions, as too many requests may result in a ban from the
website.
"""

from abc import ABC, abstractmethod
from datetime import datetime

from cinema_repertoire_analyzer.cinema_api.models import Repertoire
from cinema_repertoire_analyzer.database.models import CinemaVenuesBase


class Cinema(ABC):
    @abstractmethod
    def fetch_repertoire(self, date: datetime, cinema_venue: str) -> list[Repertoire]:
        """Download repertoire for a specified date from the cinema website."""

    @abstractmethod
    def fetch_cinema_venues_list(self) -> list[CinemaVenuesBase]:
        """Download list of cinema venues from the cinema website."""
