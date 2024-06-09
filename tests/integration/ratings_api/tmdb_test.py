import aiohttp
import pytest

from cinema_repertoire_analyzer.ratings_api.tmdb import fetch_all_movie_details, fetch_movie_details
from cinema_repertoire_analyzer.settings import Settings
from e2e.conftest import settings


@pytest.mark.integration
async def test_fetch_movie_details_successfully_returns_movie_details(settings: Settings) -> None:
    async with aiohttp.ClientSession() as session:
        response = await fetch_movie_details(
            session, "Furiosa: A Mad Max Saga", settings.USER_PREFERENCES.TMDB_ACCESS_TOKEN
        )

        assert response["results"][0]["original_title"] == "Furiosa: A Mad Max Saga"


@pytest.mark.integration
async def test_fetch_all_movie_details_successfully_returns_multiple_movies_details(
    settings: Settings,
) -> None:
    response = await fetch_all_movie_details(
        ["The Watchers", "Garfield", "Puchatek: Krew i miód 2"],
        settings.USER_PREFERENCES.TMDB_ACCESS_TOKEN,
    )

    assert len(response.items()) == 3
    assert response.keys() == {"The Watchers", "Garfield", "Puchatek: Krew i miód 2"}
