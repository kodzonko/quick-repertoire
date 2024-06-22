from http import HTTPStatus
from typing import Any

import aiohttp
import pytest
from mockito import mock, when
from requests import Response

import cinema_repertoire_analyzer.ratings_api.tmdb as tested_module


@pytest.fixture
def access_token() -> str:
    return "1234"


@pytest.fixture
def authorization_url() -> str:
    return "https://api.themoviedb.org/3/authentication"


@pytest.fixture
def ok_response() -> Response:
    response = mock(Response)
    response.status_code = HTTPStatus.OK
    return response  # type: ignore[no-any-return]


@pytest.fixture
def unauthorized_response() -> Response:
    response = mock(Response)
    response.status_code = HTTPStatus.UNAUTHORIZED
    return response  # type: ignore[no-any-return]


@pytest.fixture
def session() -> aiohttp.ClientSession:
    return mock(aiohttp.ClientSession)  # type: ignore[no-any-return]


@pytest.fixture
def no_results_response_body() -> dict[str, Any]:
    return {"page": 1, "results": [], "total_pages": 1, "total_results": 0}


@pytest.fixture
def single_result_response_body() -> dict[str, Any]:
    return {
        "page": 1,
        "results": [
            {
                "adult": False,
                "backdrop_path": "/pulJ1iY7GVeppMRipiR7ZGDW7EW.jpg",
                "genre_ids": [18],
                "id": 615,
                "original_language": "en",
                "original_title": "The Passion of the Christ",
                "overview": (
                    '"Pasja" jest wstrząsającym opisem ostatnich 12 godzin życia Jezusa Chrystusa. '
                    "To zdecydowanie najmocniejsze przedstawienie Pasji, z jakim spotkaliśmy się do"
                    " tej pory w kinie. Film jest wierny przekazom historycznym, biblijnym oraz "
                    "teologicznym. Aby podkreślić autentyczność historii, aktorzy posługują się "
                    'dwoma wymarłymi językami: aramejskim i łaciną.\r Film twórcy "Braveheart - '
                    'Waleczne Serce" wywołuje wiele emocji i kontrowersji. "Pasja" to obrazowe '
                    "dzieło sztuki prowokujące do poważnego myślenia i refleksji nad śmiercią "
                    "Chrystusa osoby o różnych przekonaniach religijnych. Jest to film o wierze, "
                    "nadziei, miłości i przebaczeniu - a więc o tym, czego bardzo potrzeba w "
                    "dzisiejszych burzliwych czasach.  [opis dystrybutora dvd]"
                ),
                "popularity": 58.187,
                "poster_path": "/xwgMHTf6BdGRbqCC8fZGuT5R6vj.jpg",
                "release_date": "2004-02-25",
                "title": "Pasja",
                "video": False,
                "vote_average": 7.504,
                "vote_count": 4445,
            }
        ],
        "total_pages": 1,
        "total_results": 1,
    }


@pytest.fixture
def multiple_results_response_body() -> dict[str, Any]:
    return {
        "page": 1,
        "results": [
            {
                "adult": False,
                "backdrop_path": "/fM736e6Za4tofRqPFguhdE3MjpO.jpg",
                "genre_ids": [80, 18, 53],
                "id": 9740,
                "original_language": "en",
                "original_title": "Hannibal",
                "overview": (
                    "Milioner Mason Verger informuje FBI, że chce przekazać na ręce Clarice "
                    "Starling materiały, które mogą pomóc w schwytaniu Hannibala Lectera. Przed "
                    "laty Verger został przez niego potwornie okaleczony. Teraz żyje tylko chęcią "
                    "krwawego odwetu na doktorze. Agentka Starling ma posłużyć do wywabienia "
                    "Lectera z kryjówki. Inspektor Pazzi wpada na trop Hannibala. Funkcjonariusz "
                    "postanawia sprzedać tę informację Vergerowi. Ten stawia jednak warunek - Pazzi"
                    " dostanie 3 miliony dolarów, jeśli dostarczy odcisk palca domniemanego "
                    "Lectera."
                ),
                "popularity": 7.96,
                "poster_path": "/hFRQ7LcCyFOdu6ZfTIZt0o0cMI5.jpg",
                "release_date": "2001-02-08",
                "title": "Hannibal",
                "video": False,
                "vote_average": 6.772,
                "vote_count": 4553,
            },
            {
                "adult": False,
                "backdrop_path": None,
                "genre_ids": [18],
                "id": 388859,
                "original_language": "en",
                "original_title": "Hannibal",
                "overview": (
                    "A young man happens upon a strange, isolated village which is "
                    "oppressively ruled by foreign soldiers. When he tries to inquire into"
                    " what is going on, he is forced to flee to an island where a renegade"
                    " medical doctor tries to force him into submission."
                ),
                "popularity": 1.582,
                "poster_path": "/3dnBp9NlOpz8LNN9yUw2qpI2YZz.jpg",
                "release_date": "1972-03-19",
                "title": "Hannibal",
                "video": False,
                "vote_average": 0,
                "vote_count": 0,
            },
        ],
        "total_pages": 1,
        "total_results": 2,
    }


@pytest.mark.unit
def test_verify_api_key_makes_request_with_successful_status_code(
    access_token: str, authorization_url: str, ok_response: Response
) -> None:
    headers = {"accept": "application/json", "Authorization": f"Bearer {access_token}"}
    when(tested_module.requests).get(authorization_url, headers=headers).thenReturn(ok_response)
    assert tested_module.verify_api_key(access_token) is True


@pytest.mark.unit
def test_verify_api_key_makes_request_with_unauthorized_status_code(
    access_token: str, authorization_url: str, unauthorized_response: Response
) -> None:
    headers = {"accept": "application/json", "Authorization": f"Bearer {access_token}"}
    when(tested_module.requests).get(authorization_url, headers=headers).thenReturn(
        unauthorized_response
    )
    assert tested_module.verify_api_key(access_token) is False


@pytest.mark.unit
def test_verify_api_key_called_without_access_token() -> None:
    assert tested_module.verify_api_key(None) is False


@pytest.mark.unit
@pytest.mark.parametrize(
    "response, outcome",
    [
        pytest.param("no_results_response_body", False),
        pytest.param("single_result_response_body", True),
        pytest.param("multiple_results_response_body", False),
    ],
)
def test_ensure_single_result_returns_correct_bool_based_on_number_of_results(
    response: dict[str, Any], outcome: bool, request: Any
) -> None:
    assert tested_module.ensure_single_result(request.getfixturevalue(response)) is outcome


@pytest.mark.unit
@pytest.mark.parametrize(
    "response, outcome",
    [
        pytest.param("no_results_response_body", "0.0/10"),
        pytest.param("single_result_response_body", "7.504/10\n(głosy: 4445)"),
        pytest.param("multiple_results_response_body", "0.0/10"),
    ],
)
def test_parse_movie_rating_parses_rating_correctly(
    response: dict[str, Any], outcome: bool, request: Any
) -> None:
    assert tested_module.parse_movie_rating(request.getfixturevalue(response)) == outcome


@pytest.mark.unit
@pytest.mark.parametrize(
    "response, outcome",
    [
        pytest.param("no_results_response_body", "Brak opisu filmu."),
        pytest.param(
            "single_result_response_body",
            (
                '"Pasja" jest wstrząsającym opisem ostatnich 12 godzin życia Jezusa '
                "Chrystusa. To zdecydowanie najmocniejsze przedstawienie Pasji, z jakim "
                "spotkaliśmy się do tej pory w kinie. Film jest wierny przekazom "
                "historycznym, biblijnym oraz teologicznym. Aby podkreślić autentyczność "
                "historii, aktorzy posługują się dwoma wymarłymi językami: aramejskim i "
                "łaciną.\r"
                ' Film twórcy "Braveheart - Waleczne Serce" wywołuje wiele emocji i '
                'kontrowersji. "Pasja" to obrazowe dzieło sztuki prowokujące do poważnego '
                "myślenia i refleksji nad śmiercią Chrystusa osoby o różnych przekonaniach "
                "religijnych. Jest to film o wierze, nadziei, miłości i przebaczeniu - a więc "
                "o tym, czego bardzo potrzeba w dzisiejszych burzliwych czasach.  [opis "
                "dystrybutora dvd]"
            ),
        ),
        pytest.param("multiple_results_response_body", "Brak opisu filmu."),
    ],
)
def test_parse_movie_summary_parses_summary_correctly(
    response: dict[str, Any], outcome: bool, request: Any
) -> None:
    assert tested_module.parse_movie_summary(request.getfixturevalue(response)) == outcome
