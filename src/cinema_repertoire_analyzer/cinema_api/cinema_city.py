import re
from datetime import datetime

from bs4 import BeautifulSoup
from requests_html import Element, HTMLResponse, HTMLSession

from cinema_repertoire_analyzer import utils
from cinema_repertoire_analyzer.cinema_api.models import (
    CinemaVenue,
    MoviePlayDetails,
    Repertoire,
)
from cinema_repertoire_analyzer.enums import CinemaChain


class CinemaCity:
    """Class handling interactions with www.cinema-city.pl website."""

    def __init__(self, repertoire_url: str, cinema_venues_url: str) -> None:
        self.cinema_chain = CinemaChain.CINEMA_CITY
        self.repertoire_url = repertoire_url
        self.cinema_venues_url = cinema_venues_url

    def fetch_repertoire(self, date: datetime, venue_id: int) -> list[Repertoire]:
        """Download repertoire for a specified date and venue from the cinema website."""
        repertoire_date = date.strftime("%Y-%m-%d")
        session = HTMLSession()
        url = utils.fill_string_template(
            self.repertoire_url,
            cinema_venue_id=venue_id,
            repertoire_date=repertoire_date,
        )
        response: HTMLResponse = session.get(url)
        response.html.render()  # render JS elements
        session.close()  # otherwise Chromium process will leak
        soup = BeautifulSoup(response.html.html, "lxml")
        output = []
        movies_details: list[Element] = soup.find_all("div", class_="qb-movie-details")
        for movie in movies_details:
            presale_header = movie.find("h4")
            is_presale = (
                presale_header is not None
                and presale_header.text == "KUP BILET W PRZEDSPRZEDAŻY"
            )
            # Presale movies in repertoire have different HTML structure
            # and are not available on selected date, so we skip.
            if not is_presale:
                output.append({
                    "title": self._parse_title(movie),
                    "genres": self._parse_genres(movie),
                    "play_length": self._parse_play_length(movie),
                    "original_language": self._parse_original_language(movie),
                    "play_details": self._parse_play_details(movie),
                })

        return output

    def fetch_cinema_venues_list(self) -> list[CinemaVenue]:
        """Download list of cinema venues from the cinema website."""
        session = HTMLSession()
        response = session.get(self.cinema_venues_url, verify=False)
        response.html.render()  # render JS elements
        cinemas = response.html.find("option[value][data-tokens]")
        venues = [cinema.element.get("data-tokens") for cinema in cinemas]
        ids = [int(cinema.element.get("value")) for cinema in cinemas]

        output: list[CinemaVenue] = []
        for venue, id_ in zip(venues, ids):
            output.append({"name": venue, "id": id_})

        return output

    def _parse_title(self, html: Element) -> str:
        """Parse HTML element of a single movie to extract title."""
        return html.find("h3", "qb-movie-name").text.strip()

    def _parse_genres(self, html: Element) -> str:
        """Parse HTML element of a single movie to extract genres."""
        raw_str = html.find("div", class_="qb-movie-info").find("span").text
        return raw_str.replace("|", "").strip()

    def _parse_original_language(self, html: Element) -> str | None:
        """Parse HTML element of a single movie to extract original language."""
        element = html.find("span", attrs={"aria-label": re.compile("original-lang")})
        return element.text.strip() if element else None

    def _parse_play_length(self, html: Element) -> int:
        """Parse HTML element of a single movie to extract play length."""
        time_raw = html.find("div", class_="qb-movie-info").find_all("span")[1].text
        time_raw = time_raw.strip()
        return int(re.sub(r"\D", "", time_raw))

    def _parse_play_format(self, html: Element) -> str:
        """Parse HTML element of a single movie to extract play format."""
        formats_section = html.find("ul", class_="qb-screening-attributes")
        try:
            formats = formats_section.find_all(
                "span", attrs={"aria-label": re.compile("Screening type")}
            )
            return " ".join([f.text.strip() for f in formats])
        except AttributeError:
            return "Brak informacji"

    def _parse_play_times(self, html: Element) -> list[str]:
        """Parse HTML element of a single movie to extract play times."""
        times = html.find_all("a", class_="btn btn-primary btn-lg")
        return [t.text.strip() for t in times]

    def _parse_play_language(self, html: Element) -> str:
        """Parse HTML element of a single movie to extract play language."""
        sub_dub_or_original_prefix = html.find(
            "span", attrs={"aria-label": re.compile("subAbbr|dubAbbr|noSubs")}
        )
        language = html.find(
            "span", attrs={"aria-label": re.compile("subbed-lang|dubbed-lang")}
        )
        try:
            return (
                f"{sub_dub_or_original_prefix.text.strip()}{': ' if language else ''}{language.text.strip() if language else ''}"
            )
        except AttributeError:
            return "Brak informacji"

    def _parse_play_details(self, html: Element) -> list[MoviePlayDetails]:
        """Parse HTML element of a single movie to extract play formats, languages and respective play times."""
        output = []
        play_details = html.find_all("div", class_="qb-movie-info-column")
        for html in play_details:
            output.append({
                "format": self._parse_play_format(html),
                "play_times": self._parse_play_times(html),
                "play_language": self._parse_play_language(html),
            })
        return output
