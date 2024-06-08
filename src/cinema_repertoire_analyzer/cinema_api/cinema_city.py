import re

from bs4 import BeautifulSoup
from pydantic_core import Url
from requests import Response
from requests_html import Element, HTMLSession

from cinema_repertoire_analyzer.cinema_api.cinema import Cinema
from cinema_repertoire_analyzer.cinema_api.models import MoviePlayDetails, Repertoire
from cinema_repertoire_analyzer.cinema_api.template_utils import fill_string_template
from cinema_repertoire_analyzer.database.models import CinemaCityVenues
from cinema_repertoire_analyzer.enums import CinemaChain


class CinemaCity(Cinema):
    """Class handling interactions with www.cinema-city.pl website."""

    def __init__(self, repertoire_url: Url, cinema_venues_url: Url) -> None:
        self.cinema_chain = CinemaChain.CINEMA_CITY
        self.repertoire_url = repertoire_url
        self.cinema_venues_url = cinema_venues_url

    def fetch_repertoire(self, date: str, venue_data: CinemaCityVenues) -> list[Repertoire]:
        """Download repertoire for a specified date and venue from the cinema website."""
        session = HTMLSession()
        url = fill_string_template(
            self.repertoire_url, cinema_venue_id=venue_data.venue_id, repertoire_date=date
        )
        response: Response = session.get(url, timeout=30)
        response.html.render(timeout=30)  # render JS elements
        session.close()  # otherwise Chromium process will leak
        soup = BeautifulSoup(response.html.html, "lxml")
        output = []
        movies_details: list[Element] = soup.find_all("div", class_="row qb-movie")
        for movie in movies_details:
            presale_header = movie.find("div", class_="qb-movie-info-column").find("h4")
            is_presale = (
                presale_header is not None and presale_header.text == "KUP BILET W PRZEDSPRZEDAŻY "
            )
            # Presale movies in repertoire have different HTML structure
            # and are not available on selected date, so we skip.
            if not is_presale:
                output.append(
                    Repertoire(
                        title=self._parse_title(movie),
                        genres=self._parse_genres(movie),
                        play_length=self._parse_play_length(movie),
                        original_language=self._parse_original_language(movie),
                        play_details=self._parse_play_details(movie),
                    )
                )

        return output

    def fetch_cinema_venues_list(self) -> list[CinemaCityVenues]:
        """Download list of cinema venues from the cinema website."""
        session = HTMLSession()
        response = session.get(self.cinema_venues_url)
        response.html.render()  # render JS elements
        cinemas = response.html.find("option[value][data-tokens]")
        venues = [cinema.element.get("data-tokens") for cinema in cinemas]
        ids = [int(cinema.element.get("value")) for cinema in cinemas]

        output: list[CinemaCityVenues] = []
        for venue, id_ in zip(venues, ids):
            output.append(CinemaCityVenues(venue_name=venue, venue_id=id_))

        return output

    def _parse_title(self, html: Element) -> str:
        """Parse HTML element of a single movie to extract title."""
        return html.find("h3", "qb-movie-name").text.strip()

    def _parse_genres(self, html: Element) -> str:
        """Parse HTML element of a single movie to extract genres."""
        try:
            raw_str = html.find("div", class_="qb-movie-info-wrapper").find("span").text
            if "|" not in raw_str:  # means no info about genres
                return "N/A"
            else:
                return raw_str.replace("|", "").strip()
        except AttributeError:
            return "N/A"

    def _parse_original_language(self, html: Element) -> str:
        """Parse HTML element of a single movie to extract original language."""
        try:
            element = html.find("span", attrs={"aria-label": re.compile("original-lang")})
            return element.text.strip()
        except AttributeError:
            return "N/A"

    def _parse_play_length(self, html: Element) -> str:
        """Parse HTML element of a single movie to extract play length."""
        try:
            target_tag = html.find("div", class_="qb-movie-info-wrapper").find(
                "span", string=re.compile(r"^\d+ min")
            )
            return target_tag.text
        except AttributeError:
            return "N/A"

    def _parse_play_format(self, html: Element) -> str:
        """Parse HTML element of a single movie to extract play format."""
        formats_section = html.find("ul", class_="qb-screening-attributes")
        try:
            formats = formats_section.find_all(
                "span", attrs={"aria-label": re.compile("Screening type")}
            )
            return " ".join([f.text.strip() for f in formats])
        except AttributeError:
            return "N/A"

    def _parse_play_times(self, html: Element) -> list[str]:
        """Parse HTML element of a single movie to extract play times."""
        times = html.find_all("a", class_="btn btn-primary btn-lg")
        parsed_times = [re.sub(r"\s+", " ", t.text) for t in times]
        parsed_times = [t.strip() for t in parsed_times]
        return parsed_times

    def _parse_play_language(self, html: Element) -> str:
        """Parse HTML element of a single movie to extract play language."""
        sub_dub_or_original_prefix = html.find(
            "span", attrs={"aria-label": re.compile("subAbbr|dubAbbr|noSubs")}
        )
        language = html.find("span", attrs={"aria-label": re.compile("subbed-lang|dubbed-lang")})
        try:
            return (
                f"{sub_dub_or_original_prefix.text.strip()}{': ' if language else ''}"
                f"{language.text.strip() if language else ''}"
            )
        except AttributeError:
            return "N/A"

    def _parse_play_details(self, html: Element) -> list[MoviePlayDetails]:
        """Parse HTML element of a single movie to extract play formats, languages and respective play times."""  # noqa: E501
        output = []
        play_details = html.find_all("div", class_="qb-movie-info-column")
        for html in play_details:
            output.append(
                MoviePlayDetails(
                    format=self._parse_play_format(html),
                    play_times=self._parse_play_times(html),
                    play_language=self._parse_play_language(html),
                )
            )
        return output
