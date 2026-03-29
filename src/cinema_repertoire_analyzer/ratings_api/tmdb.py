from datetime import datetime
from http import HTTPStatus
from typing import Any
from urllib.parse import urlencode

import anyio
import httpx

from cinema_repertoire_analyzer.ratings_api.models import TmdbMovieDetails


def verify_api_key(access_token: str | None) -> bool:
    """Verify if the API key is set in the environment variables and attempt authenticating with
    the service.
    """  # noqa: D205
    if not access_token:
        return False
    url = "https://api.themoviedb.org/3/authentication"
    headers = {"accept": "application/json", "Authorization": f"Bearer {access_token}"}
    return httpx.get(url, headers=headers, timeout=30.0).status_code == HTTPStatus.OK


async def fetch_movie_details(
    session: httpx.AsyncClient, movie_name: str, access_token: str
) -> dict[str, Any]:
    """Get details about a movie from the TMDB API."""
    base_url = "https://api.themoviedb.org/3/search/movie?"
    params = {
        "query": movie_name,
        "include_adult": True,
        "language": "pl-PL",
        "year": f"{datetime.now().year},{datetime.now().year - 1}",
        "page": 1,
    }
    # Keep query serialization aligned with existing VCR cassettes.
    url = base_url + urlencode(params, safe=":,")
    headers = {"accept": "application/json", "Authorization": f"Bearer {access_token}"}
    response = await session.get(url, headers=headers)
    return response.json()  # type: ignore[no-any-return]


async def fetch_all_movie_details(
    movie_names: list[str], access_token: str
) -> dict[str, dict[str, Any]]:
    """Get details about multiple movies from the TMDB API."""
    output: dict[str, dict[str, Any]] = {movie_name: {} for movie_name in movie_names}

    async def fetch_and_store(session: httpx.AsyncClient, movie_name: str) -> None:
        try:
            output[movie_name] = await fetch_movie_details(session, movie_name, access_token)
        except Exception:
            output[movie_name] = {}

    async with httpx.AsyncClient(timeout=30.0) as session, anyio.create_task_group() as task_group:
        for movie_name in movie_names:
            task_group.start_soon(fetch_and_store, session, movie_name)

    return output


def ensure_single_result(movie_data: dict) -> bool:
    """Ensure that there is only one result in the TMDB API response."""
    return len(movie_data["results"]) == 1


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
    movie_names: list[str], access_token: str
) -> dict[str, TmdbMovieDetails]:
    """Get ratings for a list of movies."""
    movie_data = anyio.run(fetch_all_movie_details, movie_names, access_token, backend="trio")
    output = {}
    for movie_name, data in movie_data.items():
        rating = parse_movie_rating(data)
        summary = parse_movie_summary(data)
        output[movie_name] = TmdbMovieDetails(rating=rating, summary=summary)
    return output
