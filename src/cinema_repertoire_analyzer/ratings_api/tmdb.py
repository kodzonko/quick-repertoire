from datetime import datetime
from http import HTTPStatus
from typing import Any, Protocol
from urllib.parse import urlencode

import httpx

from cinema_repertoire_analyzer.ratings_api.models import TmdbMovieDetails

REQUEST_TIMEOUT_SECONDS = 30
AUTH_URL = "https://api.themoviedb.org/3/authentication"
SEARCH_URL = "https://api.themoviedb.org/3/search/movie?"


class _SupportsGet(Protocol):
    def get(self, url: str, headers: dict[str, str], timeout: float | int) -> Any: ...


class TmdbClient:
    """Sync TMDB client used by the CLI."""

    def __init__(self, session: _SupportsGet | None = None) -> None:
        self._session = session

    def verify_api_key(self, access_token: str | None) -> bool:
        """Verify if the API key is set and valid."""
        if not access_token:
            return False
        try:
            response = self._get_authentication_response(access_token)
        except httpx.RequestError:
            return False
        return response.status_code == HTTPStatus.OK

    def fetch_movie_details(self, movie_name: str, access_token: str) -> dict[str, Any]:
        """Get details about a movie from the TMDB API."""
        url = SEARCH_URL + urlencode(self._make_search_params(movie_name), safe=":,")
        if self._session is not None:
            response = self._session.get(
                url, headers=self._make_headers(access_token), timeout=REQUEST_TIMEOUT_SECONDS
            )
        else:
            with httpx.Client(timeout=REQUEST_TIMEOUT_SECONDS) as session:
                response = session.get(url, headers=self._make_headers(access_token))
        return response.json()  # type: ignore[no-any-return]

    def fetch_all_movie_details(
        self, movie_names: list[str], access_token: str
    ) -> dict[str, dict[str, Any]]:
        """Get details about multiple movies from the TMDB API."""
        output: dict[str, dict[str, Any]] = {}
        for movie_name in movie_names:
            try:
                output[movie_name] = self.fetch_movie_details(movie_name, access_token)
            except (httpx.RequestError, ValueError):
                output[movie_name] = {}
        return output

    def get_movie_ratings_and_summaries(
        self, movie_names: list[str], access_token: str
    ) -> dict[str, TmdbMovieDetails]:
        """Get ratings for a list of movies."""
        movie_data = self.fetch_all_movie_details(movie_names, access_token)
        output = {}
        for movie_name, data in movie_data.items():
            rating = parse_movie_rating(data)
            summary = parse_movie_summary(data)
            output[movie_name] = TmdbMovieDetails(rating=rating, summary=summary)
        return output

    def _get_authentication_response(self, access_token: str) -> httpx.Response | Any:
        if self._session is not None:
            return self._session.get(
                AUTH_URL, headers=self._make_headers(access_token), timeout=REQUEST_TIMEOUT_SECONDS
            )
        return httpx.get(
            AUTH_URL, headers=self._make_headers(access_token), timeout=REQUEST_TIMEOUT_SECONDS
        )

    def _make_headers(self, access_token: str) -> dict[str, str]:
        return {"accept": "application/json", "Authorization": f"Bearer {access_token}"}

    def _make_search_params(self, movie_name: str) -> dict[str, str | bool | int]:
        current_year = datetime.now().year
        return {
            "query": movie_name,
            "include_adult": True,
            "language": "pl-PL",
            "year": f"{current_year},{current_year - 1}",
            "page": 1,
        }


def verify_api_key(access_token: str | None, client: TmdbClient | None = None) -> bool:
    """Verify if the API key is set and valid."""
    return (client or TmdbClient()).verify_api_key(access_token)


def fetch_movie_details(
    movie_name: str, access_token: str, client: TmdbClient | None = None
) -> dict[str, Any]:
    """Get details about a movie from the TMDB API."""
    return (client or TmdbClient()).fetch_movie_details(movie_name, access_token)


def fetch_all_movie_details(
    movie_names: list[str], access_token: str, client: TmdbClient | None = None
) -> dict[str, dict[str, Any]]:
    """Get details about multiple movies from the TMDB API."""
    return (client or TmdbClient()).fetch_all_movie_details(movie_names, access_token)


def ensure_single_result(movie_data: dict) -> bool:
    """Ensure that there is only one result in the TMDB API response."""
    return len(movie_data.get("results", [])) == 1


def parse_movie_rating(movie_data: dict) -> str:
    """Parse the rating of a movie from the TMDB API response."""
    try:
        if not ensure_single_result(movie_data):
            return "0.0/10"
        return (
            f"{movie_data['results'][0]['vote_average']}/10\n(głosy: "
            f"{movie_data['results'][0]['vote_count']})"
        )
    except KeyError, IndexError:
        return "0.0/10"


def parse_movie_summary(movie_data: dict) -> str:
    """Parse the summary of a movie from the TMDB API response."""
    try:
        if not ensure_single_result(movie_data):
            return "Brak opisu filmu."
        return movie_data["results"][0]["overview"]  # type: ignore[no-any-return]
    except KeyError, IndexError:
        return "Brak opisu filmu."


def get_movie_ratings_and_summaries(
    movie_names: list[str], access_token: str, client: TmdbClient | None = None
) -> dict[str, TmdbMovieDetails]:
    """Get ratings for a list of movies."""
    return (client or TmdbClient()).get_movie_ratings_and_summaries(movie_names, access_token)
