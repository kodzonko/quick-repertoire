import pytest
from mockito import mock, when
from pydantic_core import Url
from requests_html import HTML, HTMLResponse, HTMLSession

import cinema_repertoire_analyzer.cinema_api.cinema_city as tested_module
from cinema_repertoire_analyzer.cinema_api.models import MoviePlayDetails, Repertoire
from cinema_repertoire_analyzer.database.models import CinemaCityVenues
from conftest import RESOURCE_DIR


@pytest.fixture
def cinema_city() -> tested_module.CinemaCity:
    return tested_module.CinemaCity(
        repertoire_url=Url(
            "https://www.cinema-city.pl/#/buy-tickets-by-cinema?"
            "in-cinema={cinema_venue_id}&at={repertoire_date}"
        ),
        cinema_venues_url=Url("https://www.cinema-city.pl/#/buy-tickets-by-cinema"),
    )


@pytest.fixture
def session() -> HTMLSession:
    return mock(HTMLSession)


@pytest.mark.unit
def test_fetch_repertoire_downloads_and_parses_repertoire_correctly(
    cinema_city: tested_module.CinemaCity, session: HTMLSession, response: HTMLResponse
) -> None:
    when(tested_module).HTMLSession().thenReturn(session)
    when(session).get(
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema?in-cinema=1097&at=2023-04-01",
        timeout=30,
    ).thenReturn(response)
    when(response.html).render(timeout=30)
    when(session).close()
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
        Repertoire(
            title="Air",
            genres="N/A",
            play_length="N/A",
            original_language="N/A",
            play_details=[
                MoviePlayDetails(
                    format="N/A", play_language="N/A", play_times=["Śr kwi 5", "Czw kwi 6"]
                )
            ],
        ),
        Repertoire(
            title="Dungeons & Dragons: Złodziejski honor",
            genres="N/A",
            play_length="N/A",
            original_language="N/A",
            play_details=[
                MoviePlayDetails(
                    format="N/A",
                    play_language="N/A",
                    play_times=[
                        "Pon kwi 10",
                        "Wt kwi 11",
                        "Pt kwi 14",
                        "Sb kwi 15",
                        "Nie kwi 16",
                        "Pon kwi 17",
                        "Wt kwi 18",
                        "Śr kwi 19",
                        "Czw kwi 20",
                    ],
                )
            ],
        ),
        Repertoire(
            title="Super Mario Bros. Film",
            genres="N/A",
            play_length="N/A",
            original_language="N/A",
            play_details=[
                MoviePlayDetails(format="N/A", play_language="N/A", play_times=["Wt kwi 11"])
            ],
        ),
        Repertoire(
            title="Metallica: 72 Seasons – Global Premiere",
            genres="N/A",
            play_length="N/A",
            original_language="N/A",
            play_details=[
                MoviePlayDetails(format="N/A", play_language="N/A", play_times=["Czw kwi 13"])
            ],
        ),
        Repertoire(
            title="Suzume",
            genres="N/A",
            play_length="N/A",
            original_language="N/A",
            play_details=[
                MoviePlayDetails(
                    format="N/A",
                    play_language="N/A",
                    play_times=[
                        "Pt kwi 21",
                        "Sb kwi 22",
                        "Nie kwi 23",
                        "Pon kwi 24",
                        "Wt kwi 25",
                        "Śr kwi 26",
                        "Czw kwi 27",
                    ],
                )
            ],
        ),
    ]

    assert (
        cinema_city.fetch_repertoire(
            date="2023-04-01",
            venue_data=CinemaCityVenues(venue_id="1097", venue_name="Wrocław - Wroclavia"),
        )
        == expected
    )


@pytest.fixture
def response(session: HTMLSession) -> HTMLResponse:
    response = mock(HTMLResponse)
    with open(RESOURCE_DIR / "cinema_city_example_repertoire.html", encoding="utf-8") as f:
        response.html = mock(HTML)
        response.html.html = f.read()
        response.session = session
    return response
