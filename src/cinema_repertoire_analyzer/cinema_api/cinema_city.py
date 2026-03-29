import re
from contextlib import AbstractAsyncContextManager

from bs4 import BeautifulSoup
from bs4.element import NavigableString, Tag
from playwright.async_api import Browser, Playwright, async_playwright

from cinema_repertoire_analyzer.cinema_api.models import (
    CinemaChainId,
    CinemaVenue,
    MoviePlayDetails,
    Repertoire,
)
from cinema_repertoire_analyzer.cinema_api.template_utils import fill_string_template

REQUEST_TIMEOUT_SECONDS = 30
REQUEST_TIMEOUT_MILLISECONDS = REQUEST_TIMEOUT_SECONDS * 1000
REPERTOIRE_SELECTOR = "div.row.qb-movie"
CINEMA_VENUES_SELECTOR = "option[value][data-tokens]"


class CinemaCity:
    """Class handling interactions with www.cinema-city.pl website."""

    def __init__(self, repertoire_url: str, cinema_venues_url: str) -> None:
        self.repertoire_url = repertoire_url
        self.cinema_venues_url = cinema_venues_url
        self._playwright_context: AbstractAsyncContextManager[Playwright] | None = None
        self._browser: Browser | None = None

    async def __aenter__(self) -> CinemaCity:
        """Open the shared Playwright browser for this client instance."""
        await self._ensure_browser()
        return self

    async def __aexit__(self, exc_type: object, exc: object, traceback: object) -> None:
        """Close the shared Playwright browser for this client instance."""
        await self.close()

    async def close(self) -> None:
        """Close the underlying browser session when it was opened."""
        browser = self._browser
        playwright_context = self._playwright_context
        self._browser = None
        self._playwright_context = None

        try:
            if browser is not None:
                await browser.close()
        finally:
            if playwright_context is not None:
                await playwright_context.__aexit__(None, None, None)

    async def fetch_repertoire(self, date: str, venue_data: CinemaVenue) -> list[Repertoire]:
        """Download repertoire for a specified date and venue from the cinema website."""
        url = fill_string_template(
            self.repertoire_url, cinema_venue_id=venue_data.venue_id, repertoire_date=date
        )
        rendered_html = await self._fetch_rendered_html(url, REPERTOIRE_SELECTOR)
        soup = BeautifulSoup(rendered_html, "lxml")
        output = []
        movies_details: list[Tag] = soup.find_all("div", class_="row qb-movie")
        for movie in movies_details:
            presale_header = movie.find("div", class_="qb-movie-info-column").find("h4")
            is_presale = presale_header is not None and "PRZEDSPRZED" in presale_header.text.upper()
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

    async def fetch_venues(self) -> list[CinemaVenue]:
        """Download list of cinema venues from the cinema website."""
        rendered_html = await self._fetch_rendered_html(
            self.cinema_venues_url, CINEMA_VENUES_SELECTOR
        )
        soup = BeautifulSoup(rendered_html, "lxml")
        output: list[CinemaVenue] = []
        for cinema in soup.select(CINEMA_VENUES_SELECTOR):
            venue = self._get_attr_text(cinema, "data-tokens")
            venue_id = self._get_attr_text(cinema, "value")
            if not venue or venue == "null" or not venue_id.isdigit():
                continue
            output.append(
                CinemaVenue(
                    chain_id=CinemaChainId.CINEMA_CITY.value, venue_name=venue, venue_id=venue_id
                )
            )

        return output

    async def _fetch_rendered_html(self, url: str, wait_selector: str) -> str:
        """Load a page in a headless browser and return its rendered HTML."""
        created_session = self._browser is None
        browser = await self._ensure_browser()
        page = await browser.new_page(viewport={"width": 1920, "height": 1080})
        try:
            await page.goto(
                url, wait_until="domcontentloaded", timeout=REQUEST_TIMEOUT_MILLISECONDS
            )
            await page.wait_for_selector(
                wait_selector, state="attached", timeout=REQUEST_TIMEOUT_MILLISECONDS
            )
            return await page.content()
        finally:
            try:
                await page.close()
            finally:
                if created_session:
                    await self.close()

    async def _ensure_browser(self) -> Browser:
        if self._browser is not None:
            return self._browser

        launch_args = ["--disable-dev-shm-usage", "--disable-gpu", "--no-sandbox"]
        playwright_context = async_playwright()
        playwright = await playwright_context.__aenter__()
        try:
            browser = await playwright.chromium.launch(headless=True, args=launch_args)
        except BaseException:
            await playwright_context.__aexit__(None, None, None)
            raise

        self._playwright_context = playwright_context
        self._browser = browser
        return browser

    def _parse_title(self, html: Tag) -> str:
        """Parse HTML element of a single movie to extract title."""
        title = html.find("h3", class_="qb-movie-name")
        if not isinstance(title, Tag):
            return "N/A"
        return title.text.strip()

    def _parse_genres(self, html: Tag) -> str:
        """Parse HTML element of a single movie to extract genres."""
        wrapper = html.find("div", class_="qb-movie-info-wrapper")
        if not isinstance(wrapper, Tag):
            return "N/A"
        span = wrapper.find("span")
        if not isinstance(span, Tag):
            return "N/A"
        raw_str = span.text
        if "|" not in raw_str:
            return "N/A"
        return raw_str.replace("|", "").strip()

    def _parse_original_language(self, html: Tag) -> str:
        """Parse HTML element of a single movie to extract original language."""
        element = html.find("span", attrs={"aria-label": re.compile("original-lang")})
        if not isinstance(element, Tag):
            return "N/A"
        return element.text.strip()

    def _parse_play_length(self, html: Tag) -> str:
        """Parse HTML element of a single movie to extract play length."""
        wrapper = html.find("div", class_="qb-movie-info-wrapper")
        if not isinstance(wrapper, Tag):
            return "N/A"
        target_tag = wrapper.find("span", string=re.compile(r"^\d+ min"))
        if not isinstance(target_tag, Tag):
            return "N/A"
        return target_tag.text

    def _parse_play_format(self, html: Tag) -> str:
        """Parse HTML element of a single movie to extract play format."""
        formats_section = html.find("ul", class_="qb-screening-attributes")
        if not isinstance(formats_section, Tag):
            return "N/A"
        formats = formats_section.find_all(
            "span", attrs={"aria-label": re.compile("Screening type")}
        )
        return " ".join(format_.text.strip() for format_ in formats)

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
        if not isinstance(sub_dub_or_original_prefix, Tag):
            return "N/A"
        language_text = language.text.strip() if isinstance(language, Tag) else ""
        prefix = sub_dub_or_original_prefix.text.strip()
        separator = ": " if language_text else ""
        return f"{prefix}{separator}{language_text}"

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

    def _get_attr_text(self, html: Tag, attr_name: str) -> str:
        """Return a tag attribute as a normalized string."""
        value = html.get(attr_name, "")
        if isinstance(value, list):
            return " ".join(value).strip()
        if isinstance(value, NavigableString):
            return str(value).strip()
        return value.strip()
