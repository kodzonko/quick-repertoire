from unittest.mock import AsyncMock, MagicMock

import pytest
from bs4 import BeautifulSoup
from bs4.element import NavigableString, Tag

import cinema_repertoire_analyzer.cinema_api.cinema_city as tested_module
from cinema_repertoire_analyzer.cinema_api.models import MoviePlayDetails, Repertoire
from cinema_repertoire_analyzer.database.models import CinemaVenues
from conftest import RESOURCE_DIR

pytestmark = pytest.mark.anyio


@pytest.fixture
def cinema_city() -> tested_module.CinemaCity:
    return tested_module.CinemaCity(
        repertoire_url=(
            "https://www.cinema-city.pl/#/buy-tickets-by-cinema?"
            "in-cinema={cinema_venue_id}&at={repertoire_date}"
        ),
        cinema_venues_url="https://www.cinema-city.pl/#/buy-tickets-by-cinema",
    )


@pytest.fixture
def rendered_repertoire_html() -> str:
    with open(RESOURCE_DIR / "cinema_city_example_repertoire.html", encoding="utf-8") as file:
        return file.read()


def _as_tag(html: str) -> Tag:
    parsed_tag = BeautifulSoup(html, "lxml").find("div")
    assert isinstance(parsed_tag, Tag)
    return parsed_tag


@pytest.mark.unit
async def test_fetch_repertoire_downloads_and_parses_repertoire_correctly(
    cinema_city: tested_module.CinemaCity,
    rendered_repertoire_html: str,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(
        cinema_city, "_fetch_rendered_html", AsyncMock(return_value=rendered_repertoire_html)
    )
    expected = [
        Repertoire(
            title="65",
            genres="N/A",
            play_length="N/A",
            original_language="EN",
            play_details=[
                MoviePlayDetails(
                    format="2D", play_language="NAP: PL", play_times=["17:45", "19:50"]
                )
            ],
        ),
        Repertoire(
            title="Ant-Man i Osa: Kwantomania",
            genres="N/A",
            play_length="N/A",
            original_language="EN",
            play_details=[
                MoviePlayDetails(
                    format="2D", play_language="DUB: PL", play_times=["10:45", "13:20", "19:45"]
                ),
                MoviePlayDetails(
                    format="2D", play_language="NAP: PL", play_times=["15:10", "21:00"]
                ),
            ],
        ),
        Repertoire(
            title="Asteriks i Obeliks: Imperium Smoka",
            genres="N/A",
            play_length="N/A",
            original_language="N/A",
            play_details=[
                MoviePlayDetails(
                    format="2D",
                    play_language="DUB: PL",
                    play_times=["10:10", "11:30", "12:30", "14:50", "17:10"],
                )
            ],
        ),
        Repertoire(
            title="Avatar: Istota wody",
            genres="N/A",
            play_length="N/A",
            original_language="N/A",
            play_details=[
                MoviePlayDetails(
                    format="3D", play_language="DUB: PL", play_times=["15:10", "19:10"]
                )
            ],
        ),
        Repertoire(
            title="Blef doskonały",
            genres="N/A",
            play_length="N/A",
            original_language="PL",
            play_details=[
                MoviePlayDetails(format="2D", play_language="BEZ NAPISÓW", play_times=["19:00"])
            ],
        ),
        Repertoire(
            title="Creed III",
            genres="N/A",
            play_length="N/A",
            original_language="EN",
            play_details=[
                MoviePlayDetails(
                    format="2D", play_language="NAP: PL", play_times=["14:30", "16:50", "19:40"]
                ),
                MoviePlayDetails(format="VIP 2D", play_language="NAP: PL", play_times=["20:40"]),
            ],
        ),
        Repertoire(
            title="Filip",
            genres="N/A",
            play_length="N/A",
            original_language="GER",
            play_details=[
                MoviePlayDetails(
                    format="2D", play_language="NAP: PL", play_times=["11:50", "19:15"]
                ),
                MoviePlayDetails(format="VIP 2D", play_language="NAP: PL", play_times=["13:50"]),
            ],
        ),
        Repertoire(
            title="Heaven in Hell",
            genres="N/A",
            play_length="N/A",
            original_language="PL",
            play_details=[
                MoviePlayDetails(format="2D", play_language="BEZ NAPISÓW", play_times=["21:00"])
            ],
        ),
        Repertoire(
            title="I Love My Dad",
            genres="N/A",
            play_length="N/A",
            original_language="EN",
            play_details=[
                MoviePlayDetails(format="2D", play_language="NAP: PL", play_times=["16:50"])
            ],
        ),
        Repertoire(
            title="John Wick 4",
            genres="N/A",
            play_length="N/A",
            original_language="EN",
            play_details=[
                MoviePlayDetails(
                    format="2D",
                    play_language="NAP: PL",
                    play_times=["10:40", "11:40", "14:00", "16:20", "17:20", "20:40"],
                ),
                MoviePlayDetails(
                    format="4DX 2D", play_language="NAP: PL", play_times=["11:40", "17:40", "21:00"]
                ),
                MoviePlayDetails(
                    format="VIP 2D", play_language="NAP: PL", play_times=["14:50", "16:30", "19:50"]
                ),
                MoviePlayDetails(
                    format="IMAX 2D",
                    play_language="NAP: PL",
                    play_times=["15:20", "18:40", "22:00"],
                ),
            ],
        ),
        Repertoire(
            title="Kokainowy miś",
            genres="N/A",
            play_length="N/A",
            original_language="EN",
            play_details=[
                MoviePlayDetails(
                    format="2D", play_language="NAP: PL", play_times=["14:20", "21:50"]
                )
            ],
        ),
        Repertoire(
            title="Kot w butach: Ostatnie życzenie",
            genres="N/A",
            play_length="N/A",
            original_language="N/A",
            play_details=[
                MoviePlayDetails(
                    format="2D",
                    play_language="DUB: PL",
                    play_times=["10:50", "11:40", "14:00", "16:10", "18:20"],
                )
            ],
        ),
        Repertoire(
            title="Krzyk VI",
            genres="N/A",
            play_length="N/A",
            original_language="EN",
            play_details=[
                MoviePlayDetails(
                    format="2D",
                    play_language="NAP: PL",
                    play_times=["13:00", "15:30", "18:10", "20:50", "22:00"],
                ),
                MoviePlayDetails(
                    format="VIP 2D", play_language="NAP: PL", play_times=["18:30", "21:10"]
                ),
            ],
        ),
        Repertoire(
            title="Missing",
            genres="N/A",
            play_length="N/A",
            original_language="EN",
            play_details=[
                MoviePlayDetails(
                    format="2D",
                    play_language="NAP: PL",
                    play_times=["14:00", "16:30", "19:00", "21:30"],
                ),
                MoviePlayDetails(format="VIP 2D", play_language="NAP: PL", play_times=["18:10"]),
            ],
        ),
        Repertoire(
            title="Mumie",
            genres="N/A",
            play_length="N/A",
            original_language="N/A",
            play_details=[
                MoviePlayDetails(
                    format="2D", play_language="DUB: PL", play_times=["10:10", "12:15"]
                )
            ],
        ),
        Repertoire(
            title="Pokolenie Ikea",
            genres="N/A",
            play_length="N/A",
            original_language="PL",
            play_details=[
                MoviePlayDetails(format="2D", play_language="BEZ NAPISÓW", play_times=["22:00"])
            ],
        ),
        Repertoire(
            title="Puchatek: Krew i miód",
            genres="N/A",
            play_length="N/A",
            original_language="EN",
            play_details=[
                MoviePlayDetails(
                    format="2D",
                    play_language="NAP: PL",
                    play_times=["16:00", "18:00", "20:00", "22:00"],
                )
            ],
        ),
        Repertoire(
            title="Shazam! Gniew bogów",
            genres="N/A",
            play_length="N/A",
            original_language="EN",
            play_details=[
                MoviePlayDetails(format="IMAX 2D", play_language="DUB: PL", play_times=["10:00"]),
                MoviePlayDetails(
                    format="2D", play_language="DUB: PL", play_times=["11:30", "14:15", "17:00"]
                ),
                MoviePlayDetails(
                    format="2D", play_language="NAP: PL", play_times=["12:10", "20:30"]
                ),
                MoviePlayDetails(format="IMAX 2D", play_language="NAP: PL", play_times=["12:40"]),
                MoviePlayDetails(
                    format="VIP 2D", play_language="DUB: PL", play_times=["13:00", "15:50"]
                ),
                MoviePlayDetails(format="4DX 2D", play_language="DUB: PL", play_times=["15:00"]),
            ],
        ),
        Repertoire(
            title="Sundown",
            genres="N/A",
            play_length="N/A",
            original_language="EN",
            play_details=[
                MoviePlayDetails(format="2D", play_language="NAP: PL", play_times=["19:00"])
            ],
        ),
        Repertoire(
            title="Szczęście Mikołajka",
            genres="N/A",
            play_length="N/A",
            original_language="N/A",
            play_details=[
                MoviePlayDetails(
                    format="2D", play_language="DUB: PL", play_times=["11:20", "13:20"]
                )
            ],
        ),
        Repertoire(
            title="Szkoła magicznych zwierząt",
            genres="N/A",
            play_length="N/A",
            original_language="N/A",
            play_details=[
                MoviePlayDetails(
                    format="2D", play_language="DUB: PL", play_times=["11:50", "14:00"]
                )
            ],
        ),
        Repertoire(
            title="Święty",
            genres="N/A",
            play_length="N/A",
            original_language="PL",
            play_details=[
                MoviePlayDetails(format="2D", play_language="BEZ NAPISÓW", play_times=["20:20"])
            ],
        ),
        Repertoire(
            title="Tár",
            genres="N/A",
            play_length="N/A",
            original_language="EN",
            play_details=[
                MoviePlayDetails(format="2D", play_language="NAP: PL", play_times=["17:00"])
            ],
        ),
        Repertoire(
            title="W gorsecie",
            genres="N/A",
            play_length="N/A",
            original_language="GER",
            play_details=[
                MoviePlayDetails(format="2D", play_language="NAP: PL", play_times=["16:30"])
            ],
        ),
        Repertoire(
            title="Wieloryb",
            genres="N/A",
            play_length="N/A",
            original_language="EN",
            play_details=[
                MoviePlayDetails(format="2D", play_language="NAP: PL", play_times=["22:20"])
            ],
        ),
        Repertoire(
            title="Wróżka Zębuszka",
            genres="N/A",
            play_length="N/A",
            original_language="N/A",
            play_details=[
                MoviePlayDetails(
                    format="2D",
                    play_language="DUB: PL",
                    play_times=["10:00", "11:50", "13:40", "15:30", "17:20"],
                ),
                MoviePlayDetails(format="VIP 2D", play_language="DUB: PL", play_times=["12:50"]),
            ],
        ),
        Repertoire(
            title="Wszystko wszędzie naraz",
            genres="N/A",
            play_length="N/A",
            original_language="EN",
            play_details=[
                MoviePlayDetails(format="2D", play_language="NAP: PL", play_times=["19:40"])
            ],
        ),
        Repertoire(
            title="Wyrwa",
            genres="N/A",
            play_length="N/A",
            original_language="PL",
            play_details=[
                MoviePlayDetails(
                    format="2D", play_language="BEZ NAPISÓW", play_times=["14:40", "19:10", "21:20"]
                )
            ],
        ),
        Repertoire(
            title="Zadra",
            genres="N/A",
            play_length="N/A",
            original_language="PL",
            play_details=[
                MoviePlayDetails(format="2D", play_language="BEZ NAPISÓW", play_times=["14:50"])
            ],
        ),
        Repertoire(
            title="Zadziwiający kot Maurycy",
            genres="N/A",
            play_length="N/A",
            original_language="N/A",
            play_details=[
                MoviePlayDetails(
                    format="2D", play_language="DUB: PL", play_times=["10:45", "12:40"]
                )
            ],
        ),
    ]

    assert (
        await cinema_city.fetch_repertoire(
            date="2023-04-01",
            venue_data=CinemaVenues(venue_id="1097", venue_name="Wrocław - Wroclavia"),
        )
        == expected
    )


@pytest.mark.unit
async def test_fetch_cinema_venues_list_downloads_and_parses_venues_correctly(
    cinema_city: tested_module.CinemaCity, monkeypatch: pytest.MonkeyPatch
) -> None:
    rendered_venues_html = """
    <select>
      <option value="">Wybierz kino</option>
      <option value="1080" data-tokens="Lodz - Manufaktura">Lodz - Manufaktura</option>
      <option value="1097" data-tokens="Wroclaw - Wroclavia">Wroclaw - Wroclavia</option>
      <option value="9999" data-tokens="null">Ignored</option>
    </select>
    """
    monkeypatch.setattr(
        cinema_city, "_fetch_rendered_html", AsyncMock(return_value=rendered_venues_html)
    )

    venues = await cinema_city.fetch_cinema_venues_list()

    assert [(venue.venue_name, venue.venue_id) for venue in venues] == [
        ("Lodz - Manufaktura", "1080"),
        ("Wroclaw - Wroclavia", "1097"),
    ]


@pytest.mark.unit
async def test_fetch_repertoire_skips_movies_in_presale(
    cinema_city: tested_module.CinemaCity, monkeypatch: pytest.MonkeyPatch
) -> None:
    rendered_html = """
    <div class="row qb-movie">
      <div class="qb-movie-info-column"><h4>Przedsprzedaz</h4></div>
    </div>
    <div class="row qb-movie">
      <h3 class="qb-movie-name">Regular Movie</h3>
      <div class="qb-movie-info-wrapper">
        <span>Drama |Mystery</span>
        <span>95 min</span>
      </div>
      <span aria-label="original-lang">EN</span>
      <div class="qb-movie-info-column">
        <ul class="qb-screening-attributes">
          <span aria-label="Screening type">2D</span>
        </ul>
        <span aria-label="subAbbr">NAP</span>
        <span aria-label="subbed-lang">PL</span>
        <a class="btn btn-primary btn-lg">10:00</a>
      </div>
    </div>
    """
    monkeypatch.setattr(cinema_city, "_fetch_rendered_html", AsyncMock(return_value=rendered_html))

    repertoire = await cinema_city.fetch_repertoire(
        date="2023-04-01",
        venue_data=CinemaVenues(venue_id="1097", venue_name="Wroclaw - Wroclavia"),
    )

    assert [movie.title for movie in repertoire] == ["Regular Movie"]


@pytest.mark.unit
async def test_fetch_rendered_html_returns_page_source_after_waiting(
    cinema_city: tested_module.CinemaCity, monkeypatch: pytest.MonkeyPatch
) -> None:
    page = AsyncMock()
    page.content.return_value = "<html>rendered</html>"
    browser = AsyncMock()
    browser.new_page.return_value = page
    chromium = AsyncMock()
    chromium.launch.return_value = browser
    playwright = MagicMock()
    playwright.chromium = chromium
    playwright_context = AsyncMock()
    playwright_context.__aenter__.return_value = playwright
    playwright_context.__aexit__.return_value = None
    monkeypatch.setattr(
        tested_module, "async_playwright", MagicMock(return_value=playwright_context)
    )

    rendered_html = await cinema_city._fetch_rendered_html("https://example.com", "div.ready")

    assert rendered_html == "<html>rendered</html>"
    chromium.launch.assert_awaited_once_with(
        headless=True, args=["--disable-dev-shm-usage", "--disable-gpu", "--no-sandbox"]
    )
    browser.new_page.assert_awaited_once_with(viewport={"width": 1920, "height": 1080})
    page.goto.assert_awaited_once_with(
        "https://example.com",
        wait_until="domcontentloaded",
        timeout=tested_module.REQUEST_TIMEOUT_MILLISECONDS,
    )
    page.wait_for_selector.assert_awaited_once_with(
        "div.ready", state="attached", timeout=tested_module.REQUEST_TIMEOUT_MILLISECONDS
    )
    browser.close.assert_awaited_once()


@pytest.mark.unit
@pytest.mark.parametrize(
    ("html", "expected"),
    [
        pytest.param("<div></div>", "N/A"),
        pytest.param('<div><h3 class="qb-movie-name">Inception</h3></div>', "Inception"),
    ],
)
def test_parse_title_handles_missing_and_present_titles(
    cinema_city: tested_module.CinemaCity, html: str, expected: str
) -> None:
    assert cinema_city._parse_title(_as_tag(html)) == expected


@pytest.mark.unit
@pytest.mark.parametrize(
    ("html", "expected"),
    [
        pytest.param("<div></div>", "N/A"),
        pytest.param('<div><div class="qb-movie-info-wrapper"></div></div>', "N/A"),
        pytest.param(
            '<div><div class="qb-movie-info-wrapper"><span>Drama |Mystery</span></div></div>',
            "Drama Mystery",
        ),
    ],
)
def test_parse_genres_handles_missing_and_present_values(
    cinema_city: tested_module.CinemaCity, html: str, expected: str
) -> None:
    assert cinema_city._parse_genres(_as_tag(html)) == expected


@pytest.mark.unit
@pytest.mark.parametrize(
    ("html", "expected"),
    [
        pytest.param("<div></div>", "N/A"),
        pytest.param(
            '<div><div class="qb-movie-info-wrapper"><span>soon</span></div></div>', "N/A"
        ),
        pytest.param(
            '<div><div class="qb-movie-info-wrapper"><span>95 min</span></div></div>', "95 min"
        ),
    ],
)
def test_parse_play_length_handles_missing_and_present_values(
    cinema_city: tested_module.CinemaCity, html: str, expected: str
) -> None:
    assert cinema_city._parse_play_length(_as_tag(html)) == expected


@pytest.mark.unit
def test_parse_play_format_returns_na_when_section_is_missing(
    cinema_city: tested_module.CinemaCity,
) -> None:
    assert cinema_city._parse_play_format(_as_tag("<div></div>")) == "N/A"


@pytest.mark.unit
@pytest.mark.parametrize(
    ("html", "expected"),
    [
        pytest.param('<div><span aria-label="subbed-lang">PL</span></div>', "N/A"),
        pytest.param('<div><span aria-label="noSubs">ORG</span></div>', "ORG"),
        pytest.param(
            (
                '<div><span aria-label="subAbbr">NAP</span>'
                '<span aria-label="subbed-lang">PL</span></div>'
            ),
            "NAP: PL",
        ),
    ],
)
def test_parse_play_language_handles_missing_prefix_and_optional_language(
    cinema_city: tested_module.CinemaCity, html: str, expected: str
) -> None:
    assert cinema_city._parse_play_language(_as_tag(html)) == expected


@pytest.mark.unit
def test_get_attr_text_handles_lists_navigable_strings_and_plain_strings(
    cinema_city: tested_module.CinemaCity,
) -> None:
    list_holder = MagicMock()
    list_holder.get.return_value = ["VIP", "2D"]
    string_holder = MagicMock()
    string_holder.get.return_value = NavigableString(" NAP ")
    plain_holder = MagicMock()
    plain_holder.get.return_value = " 1097 "

    assert cinema_city._get_attr_text(list_holder, "data-tokens") == "VIP 2D"
    assert cinema_city._get_attr_text(string_holder, "data-tokens") == "NAP"
    assert cinema_city._get_attr_text(plain_holder, "data-tokens") == "1097"
