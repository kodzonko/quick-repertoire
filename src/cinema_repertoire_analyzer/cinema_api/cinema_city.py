import re

from bs4 import BeautifulSoup
from bs4.element import Tag
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.common.by import By
from selenium.webdriver.support.ui import WebDriverWait

from cinema_repertoire_analyzer.cinema_api.models import MoviePlayDetails, Repertoire
from cinema_repertoire_analyzer.cinema_api.template_utils import fill_string_template
from cinema_repertoire_analyzer.database.models import CinemaVenues

REQUEST_TIMEOUT_SECONDS = 30
REPERTOIRE_SELECTOR = "div.row.qb-movie"
CINEMA_VENUES_SELECTOR = "option[value][data-tokens]"


class CinemaCity:
    """Class handling interactions with www.cinema-city.pl website."""

    def __init__(self, repertoire_url: str, cinema_venues_url: str) -> None:
        self.repertoire_url = repertoire_url
        self.cinema_venues_url = cinema_venues_url

    def fetch_repertoire(self, date: str, venue_data: CinemaVenues) -> list[Repertoire]:
        """Download repertoire for a specified date and venue from the cinema website."""
        url = fill_string_template(
            self.repertoire_url, cinema_venue_id=venue_data.venue_id, repertoire_date=date
        )
        rendered_html = self._fetch_rendered_html(url, REPERTOIRE_SELECTOR)
        soup = BeautifulSoup(rendered_html, "lxml")
        output = []
        movies_details: list[Tag] = soup.find_all("div", class_="row qb-movie")
        for movie in movies_details:
            presale_header = movie.find("div", class_="qb-movie-info-column").find("h4")
            is_presale = (
                presale_header is not None
                and "PRZEDSPRZED" in presale_header.text.upper()
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

    def fetch_cinema_venues_list(self) -> list[CinemaVenues]:
        """Download list of cinema venues from the cinema website."""
        rendered_html = self._fetch_rendered_html(self.cinema_venues_url, CINEMA_VENUES_SELECTOR)
        soup = BeautifulSoup(rendered_html, "lxml")
        output: list[CinemaVenues] = []
        for cinema in soup.select(CINEMA_VENUES_SELECTOR):
            venue = cinema.get("data-tokens", "").strip()
            venue_id = cinema.get("value", "").strip()
            if not venue or venue == "null" or not venue_id.isdigit():
                continue
            output.append(CinemaVenues(venue_name=venue, venue_id=venue_id))

        return output

    def _fetch_rendered_html(self, url: str, wait_selector: str) -> str:
        """Load a page in a headless browser and return its rendered HTML."""
        options = Options()
        options.add_argument("--headless=new")
        options.add_argument("--disable-dev-shm-usage")
        options.add_argument("--disable-gpu")
        options.add_argument("--no-sandbox")
        options.add_argument("--window-size=1920,1080")

        with webdriver.Chrome(options=options) as driver:
            driver.set_page_load_timeout(REQUEST_TIMEOUT_SECONDS)
            driver.get(url)
            WebDriverWait(driver, REQUEST_TIMEOUT_SECONDS).until(
                lambda current_driver: bool(
                    current_driver.find_elements(By.CSS_SELECTOR, wait_selector)
                )
            )
            return driver.page_source

    def _parse_title(self, html: Tag) -> str:
        """Parse HTML element of a single movie to extract title."""
        return html.find("h3", "qb-movie-name").text.strip()  # type: ignore[no-any-return]

    def _parse_genres(self, html: Tag) -> str:
        """Parse HTML element of a single movie to extract genres."""
        try:
            raw_str = html.find("div", class_="qb-movie-info-wrapper").find("span").text
            if "|" not in raw_str:  # means no info about genres
                return "N/A"
            return raw_str.replace("|", "").strip()  # type: ignore[no-any-return]
        except AttributeError:
            return "N/A"

    def _parse_original_language(self, html: Tag) -> str:
        """Parse HTML element of a single movie to extract original language."""
        try:
            element = html.find("span", attrs={"aria-label": re.compile("original-lang")})
            return element.text.strip()  # type: ignore[no-any-return]
        except AttributeError:
            return "N/A"

    def _parse_play_length(self, html: Tag) -> str:
        """Parse HTML element of a single movie to extract play length."""
        try:
            target_tag = html.find("div", class_="qb-movie-info-wrapper").find(
                "span", string=re.compile(r"^\d+ min")
            )
            return target_tag.text  # type: ignore[no-any-return]
        except AttributeError:
            return "N/A"

    def _parse_play_format(self, html: Tag) -> str:
        """Parse HTML element of a single movie to extract play format."""
        formats_section = html.find("ul", class_="qb-screening-attributes")
        try:
            formats = formats_section.find_all(
                "span", attrs={"aria-label": re.compile("Screening type")}
            )
            return " ".join([format_.text.strip() for format_ in formats])
        except AttributeError:
            return "N/A"

    def _parse_play_times(self, html: Tag) -> list[str]:
        """Parse HTML element of a single movie to extract play times."""
        times = html.find_all("a", class_="btn btn-primary btn-lg")
        parsed_times = [re.sub(r"\s+", " ", time.text) for time in times]
        parsed_times = [time.strip() for time in parsed_times]
        return parsed_times

    def _parse_play_language(self, html: Tag) -> str:
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

    def _parse_play_details(self, html: Tag) -> list[MoviePlayDetails]:
        """Parse HTML element of a single movie to extract play details."""
        output = []
        play_details = html.find_all("div", class_="qb-movie-info-column")
        for play_detail in play_details:
            output.append(
                MoviePlayDetails(
                    format=self._parse_play_format(play_detail),
                    play_times=self._parse_play_times(play_detail),
                    play_language=self._parse_play_language(play_detail),
                )
            )
        return output
