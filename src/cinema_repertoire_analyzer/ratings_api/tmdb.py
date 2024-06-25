import asyncio
import urllib
from datetime import datetime
from http import HTTPStatus

import aiohttp
import async_timeout
import requests

from cinema_repertoire_analyzer.ratings_api.models import TmdbMovieDetails


def verify_api_key(access_token: str | None) -> bool:
    """Verify if the API key is set in the environment variables and attempt authenticating with
    the service.
    """  # noqa: D205
    if not access_token:
        return False
    url = "https://api.themoviedb.org/3/authentication"
    headers = {"accept": "application/json", "Authorization": f"Bearer {access_token}"}
    return requests.get(url, headers=headers).status_code == HTTPStatus.OK


async def fetch_movie_details(
    session: aiohttp.ClientSession, movie_name: str, access_token: str
) -> dict:
    """Get details about a movie from the TMDB API."""
    base_url = "https://api.themoviedb.org/3/search/movie?"
    params = {
        "query": movie_name,
        "include_adult": True,
        "language": "pl-PL",
        "year": f"{datetime.now().year},{datetime.now().year-1}",
        "page": 1,
    }
    url = base_url + urllib.parse.urlencode(params)
    headers = {"accept": "application/json", "Authorization": f"Bearer {access_token}"}
    async with async_timeout.timeout(30):
        async with session.get(url, headers=headers) as response:
            return await response.json()  # type: ignore[no-any-return]


async def fetch_all_movie_details(movie_names: list[str], access_token: str) -> dict[str, dict]:
    """Get details about multiple movies from the TMDB API."""
    async with aiohttp.ClientSession() as session:
        tasks = {}
        for movie_name in movie_names:
            task = asyncio.ensure_future(fetch_movie_details(session, movie_name, access_token))
            tasks[movie_name] = task
        await asyncio.gather(*tasks.values(), return_exceptions=True)
        output: dict[str, dict] = {}
        for movie_name, task in tasks.items():
            if task.exception():
                output[movie_name] = {}
            else:
                output[movie_name] = task.result()
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
            f"{movie_data["results"][0]["vote_average"]}/10\n(głosy: "
            f"{movie_data["results"][0]["vote_count"]})"
        )
    except (KeyError, IndexError):
        return "0.0/10"


def parse_movie_summary(movie_data: dict) -> str:
    """Parse the summary of a movie from the TMDB API response."""
    try:
        if not ensure_single_result(movie_data):
            return "Brak opisu filmu."
        return movie_data["results"][0]["overview"]  # type: ignore[no-any-return]
    except (KeyError, IndexError):
        return "Brak opisu filmu."


def get_movie_ratings_and_summaries(
    movie_names: list[str], access_token: str
) -> dict[str, TmdbMovieDetails]:
    """Get ratings for a list of movies."""
    movie_data: dict = asyncio.get_event_loop().run_until_complete(
        fetch_all_movie_details(movie_names, access_token)
    )
    output = {}
    for movie_name, data in movie_data.items():
        rating = parse_movie_rating(data)
        summary = parse_movie_summary(data)
        output[movie_name] = TmdbMovieDetails(rating=rating, summary=summary)
    return output
