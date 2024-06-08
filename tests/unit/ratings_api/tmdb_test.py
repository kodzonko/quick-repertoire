import asyncio
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
    return response


@pytest.fixture
def unauthorized_response() -> Response:
    response = mock(Response)
    response.status_code = HTTPStatus.UNAUTHORIZED
    return response


@pytest.fixture
def session() -> aiohttp.ClientSession:
    return mock(aiohttp.ClientSession)


@pytest.fixture
def movie_details_response() -> aiohttp.ClientResponse:
    response = mock({"json": lambda: {"page": 1, "results": []}}, spec=aiohttp.ClientResponse)

    coroutine = asyncio.Future()
    coroutine.set_result(response)
    return coroutine


@pytest.fixture
def no_results_response_body() -> dict[str, Any]:
    return {"page": 1, "results": [], "total_pages": 1, "total_results": 0}


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
def test_ensure_single_result_returns_true_on_single_result(
    no_results_response_body: dict[str, Any],
) -> None:
    assert tested_module.ensure_single_result(no_results_response_body) is False
