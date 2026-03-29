from http import HTTPStatus
from typing import Any
from unittest.mock import MagicMock

import httpx
import pytest

import cinema_repertoire_analyzer.ratings_api.tmdb as tested_module
from cinema_repertoire_analyzer.ratings_api.models import TmdbMovieDetails


class DummyResponse:
    def __init__(
        self, status_code: int = HTTPStatus.OK, payload: dict[str, Any] | None = None
    ) -> None:
        self.status_code = status_code
        self._payload = payload or {}

    def json(self) -> dict[str, Any]:
        return self._payload


class DummySession:
    def __init__(self, responses: list[DummyResponse | Exception]) -> None:
        self.responses = responses
        self.calls: list[dict[str, Any]] = []

    def __enter__(self) -> DummySession:
        return self

    def __exit__(self, exc_type: Any, exc: Any, traceback: Any) -> None:
        return None

    def get(self, url: str, headers: dict[str, str], timeout: float | int) -> DummyResponse:
        self.calls.append({"url": url, "headers": headers, "timeout": timeout})
        response = self.responses.pop(0)
        if isinstance(response, Exception):
            raise response
        return response


@pytest.fixture
def access_token() -> str:
    return "1234"


@pytest.fixture
def no_results_response_body() -> dict[str, Any]:
    return {"page": 1, "results": [], "total_pages": 1, "total_results": 0}


@pytest.fixture
def single_result_response_body() -> dict[str, Any]:
    return {
        "page": 1,
        "results": [{"overview": "Opis filmu.", "vote_average": 7.504, "vote_count": 4445}],
        "total_pages": 1,
        "total_results": 1,
    }


@pytest.fixture
def multiple_results_response_body() -> dict[str, Any]:
    return {
        "page": 1,
        "results": [
            {"overview": "Opis 1", "vote_average": 6.772, "vote_count": 4553},
            {"overview": "Opis 2", "vote_average": 0, "vote_count": 0},
        ],
        "total_pages": 1,
        "total_results": 2,
    }


@pytest.mark.unit
def test_verify_api_key_returns_true_for_successful_response(access_token: str) -> None:
    client = tested_module.TmdbClient(
        session=DummySession([DummyResponse(status_code=HTTPStatus.OK)])
    )

    assert client.verify_api_key(access_token) is True


@pytest.mark.unit
def test_verify_api_key_returns_false_for_unsuccessful_response(access_token: str) -> None:
    client = tested_module.TmdbClient(
        session=DummySession([DummyResponse(status_code=HTTPStatus.UNAUTHORIZED)])
    )

    assert client.verify_api_key(access_token) is False


@pytest.mark.unit
def test_verify_api_key_returns_false_without_access_token() -> None:
    client = tested_module.TmdbClient(session=DummySession([]))

    assert client.verify_api_key(None) is False


@pytest.mark.unit
def test_verify_api_key_returns_false_when_request_fails(access_token: str) -> None:
    request = httpx.Request("GET", tested_module.AUTH_URL)
    client = tested_module.TmdbClient(
        session=DummySession([httpx.ConnectError("boom", request=request)])
    )

    assert client.verify_api_key(access_token) is False


@pytest.mark.unit
@pytest.mark.parametrize(
    ("response", "outcome"),
    [
        pytest.param("no_results_response_body", False),
        pytest.param("single_result_response_body", True),
        pytest.param("multiple_results_response_body", False),
    ],
)
def test_ensure_single_result_returns_correct_bool_based_on_number_of_results(
    response: str, outcome: bool, request: pytest.FixtureRequest
) -> None:
    assert tested_module.ensure_single_result(request.getfixturevalue(response)) is outcome


@pytest.mark.unit
def test_fetch_movie_details_returns_response_body(access_token: str) -> None:
    session = DummySession([DummyResponse(payload={"results": [{"title": "Garfield"}]})])
    client = tested_module.TmdbClient(session=session)

    assert client.fetch_movie_details("Garfield", access_token) == {
        "results": [{"title": "Garfield"}]
    }
    assert session.calls[0]["headers"]["Authorization"] == f"Bearer {access_token}"


@pytest.mark.unit
def test_fetch_all_movie_details_handles_request_failures(access_token: str) -> None:
    request = httpx.Request("GET", tested_module.SEARCH_URL)
    session = DummySession(
        [
            DummyResponse(payload={"results": [{"title": "Garfield"}]}),
            httpx.ConnectError("boom", request=request),
        ]
    )
    client = tested_module.TmdbClient(session=session)

    assert client.fetch_all_movie_details(["Garfield", "Hannibal"], access_token) == {
        "Garfield": {"results": [{"title": "Garfield"}]},
        "Hannibal": {},
    }


@pytest.mark.unit
def test_fetch_all_movie_details_deduplicates_movie_titles(access_token: str) -> None:
    session = DummySession([DummyResponse(payload={"results": [{"title": "Garfield"}]})])
    client = tested_module.TmdbClient(session=session)

    assert client.fetch_all_movie_details(["Garfield", "Garfield"], access_token) == {
        "Garfield": {"results": [{"title": "Garfield"}]}
    }
    assert len(session.calls) == 1


@pytest.mark.unit
def test_fetch_all_movie_details_reuses_single_http_client(
    access_token: str, monkeypatch: pytest.MonkeyPatch
) -> None:
    session = DummySession(
        [
            DummyResponse(payload={"results": [{"title": "Garfield"}]}),
            DummyResponse(payload={"results": [{"title": "Hannibal"}]}),
        ]
    )
    client_factory = MagicMock(return_value=session)
    monkeypatch.setattr(tested_module.httpx, "Client", client_factory)

    client = tested_module.TmdbClient()

    assert client.fetch_all_movie_details(["Garfield", "Hannibal"], access_token) == {
        "Garfield": {"results": [{"title": "Garfield"}]},
        "Hannibal": {"results": [{"title": "Hannibal"}]},
    }
    client_factory.assert_called_once_with(timeout=tested_module.REQUEST_TIMEOUT_SECONDS)


@pytest.mark.unit
def test_fetch_all_movie_details_raises_for_unauthorized_response(access_token: str) -> None:
    session = DummySession([DummyResponse(status_code=HTTPStatus.UNAUTHORIZED)])
    client = tested_module.TmdbClient(session=session)

    with pytest.raises(httpx.HTTPStatusError):
        client.fetch_all_movie_details(["Garfield"], access_token)


@pytest.mark.unit
def test_fetch_all_movie_details_raises_when_all_requests_fail(access_token: str) -> None:
    request = httpx.Request("GET", tested_module.SEARCH_URL)
    session = DummySession([httpx.ConnectError("boom", request=request)])
    client = tested_module.TmdbClient(session=session)

    with pytest.raises(httpx.RequestError):
        client.fetch_all_movie_details(["Garfield"], access_token)


@pytest.mark.unit
@pytest.mark.parametrize(
    ("response", "outcome"),
    [
        pytest.param("no_results_response_body", "0.0/10"),
        pytest.param("single_result_response_body", "7.504/10\n(głosy: 4445)"),
        pytest.param("multiple_results_response_body", "0.0/10"),
    ],
)
def test_parse_movie_rating_parses_rating_correctly(
    response: str, outcome: str, request: pytest.FixtureRequest
) -> None:
    assert tested_module.parse_movie_rating(request.getfixturevalue(response)) == outcome


@pytest.mark.unit
@pytest.mark.parametrize(
    ("response", "outcome"),
    [
        pytest.param("no_results_response_body", "Brak opisu filmu."),
        pytest.param("single_result_response_body", "Opis filmu."),
        pytest.param("multiple_results_response_body", "Brak opisu filmu."),
    ],
)
def test_parse_movie_summary_parses_summary_correctly(
    response: str, outcome: str, request: pytest.FixtureRequest
) -> None:
    assert tested_module.parse_movie_summary(request.getfixturevalue(response)) == outcome


@pytest.mark.unit
def test_parse_movie_rating_returns_default_for_malformed_payload() -> None:
    assert tested_module.parse_movie_rating({"results": [{}]}) == "0.0/10"


@pytest.mark.unit
def test_parse_movie_summary_returns_default_for_malformed_payload() -> None:
    assert tested_module.parse_movie_summary({"results": [{}]}) == "Brak opisu filmu."


@pytest.mark.unit
def test_get_movie_ratings_and_summaries_maps_results_to_models(access_token: str) -> None:
    client = tested_module.TmdbClient(
        session=DummySession(
            [
                DummyResponse(
                    payload={
                        "results": [
                            {"overview": "Opis filmu.", "vote_average": 7.5, "vote_count": 5}
                        ]
                    }
                )
            ]
        )
    )

    assert client.get_movie_ratings_and_summaries(["Garfield"], access_token) == {
        "Garfield": TmdbMovieDetails(rating="7.5/10\n(głosy: 5)", summary="Opis filmu.")
    }
