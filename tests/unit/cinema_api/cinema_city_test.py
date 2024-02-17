from datetime import datetime

import pytest
from mockito import mock, when
from requests_html import HTML, HTMLResponse, HTMLSession

import cinema_repertoire_analyzer.cinema_api.cinema_city as tested_module
from conftest import RESOURCE_DIR


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
    with open(RESOURCE_DIR / "cinema_city_example_repertoire.html", encoding="utf-8") as f:
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
    expected = [
        {
            "genres": "Sci-Fi, Thriller",
            "original_language": "EN",
            "play_details": [
                {"format": "2D", "play_language": "NAP: PL", "play_times": ["17:45", "19:50"]}
            ],
            "play_length": 90,
            "title": "65",
        },
        {
            "genres": "Akcja, Sci-Fi",
            "original_language": "EN",
            "play_details": [
                {
                    "format": "2D",
                    "play_language": "DUB: PL",
                    "play_times": ["10:45", "13:20", "19:45"],
                },
                {"format": "2D", "play_language": "NAP: PL", "play_times": ["15:10", "21:00"]},
            ],
            "play_length": 125,
            "title": "Ant-Man i Osa: Kwantomania",
        },
        {
            "genres": "Przygodowy, Komedia",
            "original_language": None,
            "play_details": [
                {
                    "format": "2D",
                    "play_language": "DUB: PL",
                    "play_times": ["10:10", "11:30", "12:30", "14:50", "17:10"],
                }
            ],
            "play_length": 111,
            "title": "Asteriks i Obeliks: Imperium Smoka",
        },
        {
            "genres": "Sci-Fi",
            "original_language": None,
            "play_details": [
                {"format": "3D", "play_language": "DUB: PL", "play_times": ["15:10", "19:10"]}
            ],
            "play_length": 193,
            "title": "Avatar: Istota wody",
        },
        {
            "genres": "Komedia",
            "original_language": "PL",
            "play_details": [
                {"format": "2D", "play_language": "BEZ NAPISÓW", "play_times": ["19:00"]}
            ],
            "play_length": 81,
            "title": "Blef doskonały",
        },
        {
            "genres": "Dramat, Sportowy",
            "original_language": "EN",
            "play_details": [
                {
                    "format": "2D",
                    "play_language": "NAP: PL",
                    "play_times": ["14:30", "16:50", "19:40"],
                },
                {"format": "VIP 2D", "play_language": "NAP: PL", "play_times": ["20:40"]},
            ],
            "play_length": 114,
            "title": "Creed III",
        },
        {
            "genres": "Dramat, Wojenny",
            "original_language": "GER",
            "play_details": [
                {"format": "2D", "play_language": "NAP: PL", "play_times": ["11:50", "19:15"]},
                {"format": "VIP 2D", "play_language": "NAP: PL", "play_times": ["13:50"]},
            ],
            "play_length": 125,
            "title": "Filip",
        },
        {
            "genres": "Romantyczny",
            "original_language": "PL",
            "play_details": [
                {"format": "2D", "play_language": "BEZ NAPISÓW", "play_times": ["21:00"]}
            ],
            "play_length": 119,
            "title": "Heaven in Hell",
        },
        {
            "genres": "Komedia, Dramat, Romantyczny",
            "original_language": "EN",
            "play_details": [{"format": "2D", "play_language": "NAP: PL", "play_times": ["16:50"]}],
            "play_length": 96,
            "title": "I Love My Dad",
        },
        {
            "genres": "Akcja, Thriller",
            "original_language": "EN",
            "play_details": [
                {
                    "format": "2D",
                    "play_language": "NAP: PL",
                    "play_times": ["10:40", "11:40", "14:00", "16:20", "17:20", "20:40"],
                },
                {
                    "format": "4DX 2D",
                    "play_language": "NAP: PL",
                    "play_times": ["11:40", "17:40", "21:00"],
                },
                {
                    "format": "VIP 2D",
                    "play_language": "NAP: PL",
                    "play_times": ["14:50", "16:30", "19:50"],
                },
                {
                    "format": "IMAX 2D",
                    "play_language": "NAP: PL",
                    "play_times": ["15:20", "18:40", "22:00"],
                },
            ],
            "play_length": 169,
            "title": "John Wick 4",
        },
        {
            "genres": "Thriller",
            "original_language": "EN",
            "play_details": [
                {"format": "2D", "play_language": "NAP: PL", "play_times": ["14:20", "21:50"]}
            ],
            "play_length": 95,
            "title": "Kokainowy miś",
        },
        {
            "genres": "Animowany",
            "original_language": None,
            "play_details": [
                {
                    "format": "2D",
                    "play_language": "DUB: PL",
                    "play_times": ["10:50", "11:40", "14:00", "16:10", "18:20"],
                }
            ],
            "play_length": 100,
            "title": "Kot w butach: Ostatnie życzenie",
        },
        {
            "genres": "Horror",
            "original_language": "EN",
            "play_details": [
                {
                    "format": "2D",
                    "play_language": "NAP: PL",
                    "play_times": ["13:00", "15:30", "18:10", "20:50", "22:00"],
                },
                {"format": "VIP 2D", "play_language": "NAP: PL", "play_times": ["18:30", "21:10"]},
            ],
            "play_length": 122,
            "title": "Krzyk VI",
        },
        {
            "genres": "Dramat, Thriller",
            "original_language": "EN",
            "play_details": [
                {
                    "format": "2D",
                    "play_language": "NAP: PL",
                    "play_times": ["14:00", "16:30", "19:00", "21:30"],
                },
                {"format": "VIP 2D", "play_language": "NAP: PL", "play_times": ["18:10"]},
            ],
            "play_length": 111,
            "title": "Missing",
        },
        {
            "genres": "Animowany",
            "original_language": None,
            "play_details": [
                {"format": "2D", "play_language": "DUB: PL", "play_times": ["10:10", "12:15"]}
            ],
            "play_length": 88,
            "title": "Mumie",
        },
        {
            "genres": "Romantyczny",
            "original_language": "PL",
            "play_details": [
                {"format": "2D", "play_language": "BEZ NAPISÓW", "play_times": ["22:00"]}
            ],
            "play_length": 100,
            "title": "Pokolenie Ikea",
        },
        {
            "genres": "Horror",
            "original_language": "EN",
            "play_details": [
                {
                    "format": "2D",
                    "play_language": "NAP: PL",
                    "play_times": ["16:00", "18:00", "20:00", "22:00"],
                }
            ],
            "play_length": 85,
            "title": "Puchatek: Krew i miód",
        },
        {
            "genres": "Akcja, Fantasy",
            "original_language": "EN",
            "play_details": [
                {"format": "IMAX 2D", "play_language": "DUB: PL", "play_times": ["10:00"]},
                {
                    "format": "2D",
                    "play_language": "DUB: PL",
                    "play_times": ["11:30", "14:15", "17:00"],
                },
                {"format": "2D", "play_language": "NAP: PL", "play_times": ["12:10", "20:30"]},
                {"format": "IMAX 2D", "play_language": "NAP: PL", "play_times": ["12:40"]},
                {"format": "VIP 2D", "play_language": "DUB: PL", "play_times": ["13:00", "15:50"]},
                {"format": "4DX 2D", "play_language": "DUB: PL", "play_times": ["15:00"]},
            ],
            "play_length": 130,
            "title": "Shazam! Gniew bogów",
        },
        {
            "genres": "Dramat",
            "original_language": "EN",
            "play_details": [{"format": "2D", "play_language": "NAP: PL", "play_times": ["19:00"]}],
            "play_length": 83,
            "title": "Sundown",
        },
        {
            "genres": "Animowany, Familijny",
            "original_language": None,
            "play_details": [
                {"format": "2D", "play_language": "DUB: PL", "play_times": ["11:20", "13:20"]}
            ],
            "play_length": 82,
            "title": "Szczęście Mikołajka",
        },
        {
            "genres": "Przygodowy, Familijny",
            "original_language": None,
            "play_details": [
                {"format": "2D", "play_language": "DUB: PL", "play_times": ["11:50", "14:00"]}
            ],
            "play_length": 93,
            "title": "Szkoła magicznych zwierząt",
        },
        {
            "genres": "Kryminał",
            "original_language": "PL",
            "play_details": [
                {"format": "2D", "play_language": "BEZ NAPISÓW", "play_times": ["20:20"]}
            ],
            "play_length": 110,
            "title": "Święty",
        },
        {
            "genres": "Dramat",
            "original_language": "EN",
            "play_details": [{"format": "2D", "play_language": "NAP: PL", "play_times": ["17:00"]}],
            "play_length": 158,
            "title": "Tár",
        },
        {
            "genres": "Dramat",
            "original_language": "GER",
            "play_details": [{"format": "2D", "play_language": "NAP: PL", "play_times": ["16:30"]}],
            "play_length": 113,
            "title": "W gorsecie",
        },
        {
            "genres": "Dramat",
            "original_language": "EN",
            "play_details": [{"format": "2D", "play_language": "NAP: PL", "play_times": ["22:20"]}],
            "play_length": 118,
            "title": "Wieloryb",
        },
        {
            "genres": "Animowany, Komedia, Fantasy",
            "original_language": None,
            "play_details": [
                {
                    "format": "2D",
                    "play_language": "DUB: PL",
                    "play_times": ["10:00", "11:50", "13:40", "15:30", "17:20"],
                },
                {"format": "VIP 2D", "play_language": "DUB: PL", "play_times": ["12:50"]},
            ],
            "play_length": 80,
            "title": "Wróżka Zębuszka",
        },
        {
            "genres": "Akcja, Komedia, Sci-Fi",
            "original_language": "EN",
            "play_details": [{"format": "2D", "play_language": "NAP: PL", "play_times": ["19:40"]}],
            "play_length": 150,
            "title": "Wszystko wszędzie naraz",
        },
        {
            "genres": "Thriller",
            "original_language": "PL",
            "play_details": [
                {
                    "format": "2D",
                    "play_language": "BEZ NAPISÓW",
                    "play_times": ["14:40", "19:10", "21:20"],
                }
            ],
            "play_length": 100,
            "title": "Wyrwa",
        },
        {
            "genres": "",
            "original_language": "PL",
            "play_details": [
                {"format": "2D", "play_language": "BEZ NAPISÓW", "play_times": ["14:50"]}
            ],
            "play_length": 88,
            "title": "Zadra",
        },
        {
            "genres": "Animowany, Komedia, Fantasy",
            "original_language": None,
            "play_details": [
                {"format": "2D", "play_language": "DUB: PL", "play_times": ["10:45", "12:40"]}
            ],
            "play_length": 93,
            "title": "Zadziwiający kot Maurycy",
        },
        {
            "genres": "Dramat, Sportowy",
            "original_language": None,
            "play_details": [
                {
                    "format": "Brak informacji",
                    "play_language": "Brak informacji",
                    "play_times": ["Śr kwi 5", "Czw kwi 6"],
                }
            ],
            "play_length": 112,
            "title": "Air",
        },
        {
            "genres": "Przygodowy, Fantasy",
            "original_language": None,
            "play_details": [
                {
                    "format": "Brak informacji",
                    "play_language": "Brak informacji",
                    "play_times": [
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
                }
            ],
            "play_length": 134,
            "title": "Dungeons & Dragons: Złodziejski honor",
        },
        {
            "genres": "Przygodowy, Animowany, Komedia",
            "original_language": None,
            "play_details": [
                {
                    "format": "Brak informacji",
                    "play_language": "Brak informacji",
                    "play_times": ["Wt kwi 11"],
                }
            ],
            "play_length": 92,
            "title": "Super Mario Bros. Film",
        },
        {
            "genres": "",
            "original_language": None,
            "play_details": [
                {
                    "format": "Brak informacji",
                    "play_language": "Brak informacji",
                    "play_times": ["Czw kwi 13"],
                }
            ],
            "play_length": 120,
            "title": "Metallica: 72 Seasons – Global Premiere",
        },
        {
            "genres": "Przygodowy",
            "original_language": None,
            "play_details": [
                {
                    "format": "Brak informacji",
                    "play_language": "Brak informacji",
                    "play_times": [
                        "Pt kwi 21",
                        "Sb kwi 22",
                        "Nie kwi 23",
                        "Pon kwi 24",
                        "Wt kwi 25",
                        "Śr kwi 26",
                        "Czw kwi 27",
                    ],
                }
            ],
            "play_length": 122,
            "title": "Suzume",
        },
    ]

    assert cinema_city.fetch_repertoire(date=datetime(2023, 4, 1), venue_id=1097) == expected
