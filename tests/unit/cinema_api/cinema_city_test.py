from datetime import datetime

import pytest
from mockito import mock, when
from requests_html import HTML, HTMLResponse, HTMLSession

import cinema_repertoire_analyzer.cinema_api.cinema_city as tested_module
from unit.conftest import RESOURCE_DIR


@pytest.fixture
def cinema_city() -> tested_module.CinemaCity:
    return tested_module.CinemaCity(
        repertoire_url="https://www.cinema-city.pl/#/buy-tickets-by-cinema?in-cinema={cinema_venue_id}&at={repertoire_date}",
        cinema_venues_url="https://www.cinema-city.pl/#/buy-tickets-by-cinema",
    )


@pytest.fixture
def session() -> HTMLSession:
    return mock(HTMLSession)


@pytest.fixture
def response(session: HTMLSession) -> HTMLResponse:
    response = mock(HTMLResponse)
    with open(
        RESOURCE_DIR / "cinema_city_example_repertoire.html", encoding="utf-8"
    ) as f:
        response.html = mock(HTML)
        response.html.html = f.read()
        response.session = session
    return response


@pytest.mark.unit
def test_fetch_repertoire_downloads_and_parses_repertoire_correctly(
    cinema_city: tested_module.CinemaCity, session: HTMLSession, response: HTMLResponse
) -> None:
    when(tested_module).HTMLSession().thenReturn(session)
    when(session).get(
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema?in-cinema=1097&at=2023-04-01"
    ).thenReturn(response)
    when(response.html).render()
    when(session).close()
    expected = []

    assert (
        cinema_city.fetch_repertoire(date=datetime(2023, 4, 1), venue_id=1097)
        == expected
    )
