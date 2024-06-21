import aiohttp
import pytest

from cinema_repertoire_analyzer.ratings_api.models import TmdbMovieDetails
from cinema_repertoire_analyzer.ratings_api.tmdb import (
    fetch_all_movie_details,
    fetch_movie_details,
    get_movie_ratings_and_summaries,
)
from cinema_repertoire_analyzer.settings import Settings


@pytest.fixture
def tmdb_movies_details_dict() -> dict[str, TmdbMovieDetails]:
    return {
        "Garfield": TmdbMovieDetails(
            rating="6.717/10\n(głosy: 184)",
            summary=(
                "Garfield jest najbardziej znanym kotem na świecie. Jest leniuchem, który "
                "uwielbia lasagne i nie cierpi poniedziałków. Po nieoczekiwanym spotkaniu ze "
                "swoim dawno zaginionym ojcem – niechlujnym ulicznym kotem Vicem – Garfield i "
                "jego psi przyjaciel Odie zmuszeni są porzucić swoje doskonałe leniwe życie i "
                "dołączyć do Vica w zabawnym napadzie."
            ),
        ),
        "Furiosa: Saga Mad Max": TmdbMovieDetails(
            rating="7.631/10\n(głosy: 967)",
            summary="Kiedy świat upada, młoda Furiosa zostaje uprowadzona z Zielonego Miejsca Wielu"
            " Matek. Wpada w ręce potężnej Hordy Bikerów, której przewodzi watażka Dementus"
            ". Po przebyciu Pustkowi porywacze docierają do Cytadeli, gdzie rządzi Wieczny "
            "Joe. Dwóch tyranów zaczyna walkę o władzę, Furiosa zaś musi przetrwać wiele "
            "prób, jednocześnie gromadząc środki, które pozwolą jej wrócić do domu.",
        ),
    }


@pytest.mark.integration
@pytest.mark.vcr()
async def test_fetch_movie_details_successfully_returns_movie_details(settings: Settings) -> None:
    async with aiohttp.ClientSession() as session:
        response = await fetch_movie_details(
            session, "Furiosa: A Mad Max Saga", settings.USER_PREFERENCES.TMDB_ACCESS_TOKEN
        )

        assert response["results"][0]["original_title"] == "Furiosa: A Mad Max Saga"


@pytest.mark.integration
@pytest.mark.vcr()
async def test_fetch_all_movie_details_successfully_returns_multiple_movies_details(
    settings: Settings,
) -> None:
    response = await fetch_all_movie_details(
        ["The Watchers", "Garfield", "Puchatek: Krew i miód 2"],
        settings.USER_PREFERENCES.TMDB_ACCESS_TOKEN,
    )

    assert len(response.items()) == 3
    assert response.keys() == {"The Watchers", "Garfield", "Puchatek: Krew i miód 2"}


@pytest.mark.integration
@pytest.mark.vcr()
def test_get_movie_ratings_and_summaries_returns_correct_tmdb_movie_details(
    settings: Settings, tmdb_movies_details_dict: dict[str, TmdbMovieDetails]
) -> None:
    assert (
        get_movie_ratings_and_summaries(
            ["Garfield", "Furiosa: Saga Mad Max"], settings.USER_PREFERENCES.TMDB_ACCESS_TOKEN
        )
        == tmdb_movies_details_dict
    )
