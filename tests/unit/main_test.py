from http import HTTPStatus
from unittest.mock import AsyncMock, MagicMock

import httpx
import pytest
from typer import Typer
from typer.testing import CliRunner

import cinema_repertoire_analyzer.main as tested_module
from cinema_repertoire_analyzer.cinema_api.models import (
    CinemaChainId,
    CinemaVenue,
    MoviePlayDetails,
    Repertoire,
)
from cinema_repertoire_analyzer.cinema_api.registry import RegisteredCinemaChain
from cinema_repertoire_analyzer.exceptions import AmbiguousVenueMatchError, VenueNotFoundError
from cinema_repertoire_analyzer.ratings_api.models import TmdbMovieDetails
from cinema_repertoire_analyzer.settings import Settings


@pytest.fixture
def runner() -> CliRunner:
    return CliRunner()


@pytest.fixture
def venue() -> CinemaVenue:
    return CinemaVenue(chain_id="cinema-city", venue_name="Wroclaw - Wroclavia", venue_id="1097")


@pytest.fixture
def repertoire() -> list[Repertoire]:
    return [
        Repertoire(
            title="Test Movie",
            genres="Thriller",
            play_length="120 min",
            original_language="EN",
            play_details=[
                MoviePlayDetails(
                    format="2D", play_language="NAP: PL", play_times=["10:00", "12:30"]
                )
            ],
        )
    ]


def build_registered_chain(
    client: object, default_venue_name: str | None = "Wroclaw - Wroclavia"
) -> RegisteredCinemaChain:
    return RegisteredCinemaChain(
        chain_id=CinemaChainId.CINEMA_CITY,
        display_name="Cinema City",
        default_venue_getter=lambda _settings: default_venue_name,
        client_factory=lambda _settings: client,
    )


@pytest.mark.unit
def test_resolve_single_venue_returns_single_match() -> None:
    venue = CinemaVenue(chain_id="cinema-city", venue_name="Warszawa - Janki", venue_id="1")

    assert tested_module._resolve_single_venue([venue]) == venue


@pytest.mark.unit
def test_resolve_single_venue_raises_not_found_for_empty_result() -> None:
    with pytest.raises(VenueNotFoundError):
        tested_module._resolve_single_venue([])


@pytest.mark.unit
def test_resolve_single_venue_raises_ambiguous_for_multiple_matches() -> None:
    with pytest.raises(AmbiguousVenueMatchError):
        tested_module._resolve_single_venue(
            [
                CinemaVenue(chain_id="cinema-city", venue_name="Warszawa - Janki", venue_id="1"),
                CinemaVenue(chain_id="cinema-city", venue_name="Warszawa - Arkadia", venue_id="2"),
            ]
        )


@pytest.mark.unit
def test_make_app_uses_get_settings_when_settings_are_not_provided(
    monkeypatch: pytest.MonkeyPatch, settings: Settings
) -> None:
    get_settings_mock = MagicMock(return_value=settings)
    monkeypatch.setattr(tested_module, "get_settings", get_settings_mock)
    monkeypatch.setattr(tested_module, "_build_database_manager", lambda *_: MagicMock())
    monkeypatch.setattr(tested_module, "Console", lambda: MagicMock())

    app = tested_module.make_app()

    get_settings_mock.assert_called_once_with()
    assert isinstance(app, Typer)


@pytest.mark.unit
def test_repertoire_command_exits_for_unsupported_chain(
    monkeypatch: pytest.MonkeyPatch, settings: Settings, runner: CliRunner
) -> None:
    monkeypatch.setattr(tested_module, "_build_database_manager", lambda *_: MagicMock())
    monkeypatch.setattr(tested_module, "Console", lambda: MagicMock())

    app = tested_module.make_app(settings)
    result = runner.invoke(app, ["repertoire", "--chain", "unsupported-chain"])

    assert result.exit_code == 1
    assert "Nieobsługiwana sieć kin" in result.stdout


@pytest.mark.unit
def test_repertoire_command_exits_for_ambiguous_venue_name(
    monkeypatch: pytest.MonkeyPatch, settings: Settings, runner: CliRunner
) -> None:
    fake_db_manager = MagicMock()
    fake_db_manager.find_venues_by_name.return_value = [
        CinemaVenue(chain_id="cinema-city", venue_name="Venue A", venue_id="1"),
        CinemaVenue(chain_id="cinema-city", venue_name="Venue B", venue_id="2"),
    ]
    monkeypatch.setattr(tested_module, "_build_database_manager", lambda *_: fake_db_manager)
    monkeypatch.setattr(tested_module, "Console", lambda: MagicMock())

    app = tested_module.make_app(settings)
    result = runner.invoke(app, ["repertoire", "--chain", "cinema-city", "venue"])

    assert result.exit_code == 1
    assert "Podana nazwa lokalu jest niejednoznaczna." in result.stdout


@pytest.mark.unit
def test_repertoire_command_exits_for_missing_venue_name(
    monkeypatch: pytest.MonkeyPatch, settings: Settings, runner: CliRunner
) -> None:
    fake_db_manager = MagicMock()
    fake_db_manager.find_venues_by_name.return_value = []
    monkeypatch.setattr(tested_module, "_build_database_manager", lambda *_: fake_db_manager)
    monkeypatch.setattr(tested_module, "Console", lambda: MagicMock())

    app = tested_module.make_app(settings)
    result = runner.invoke(app, ["repertoire", "--chain", "cinema-city", "venue"])

    assert result.exit_code == 1
    assert "Nie znaleziono żadnego lokalu o podanej nazwie." in result.stdout


@pytest.mark.unit
def test_repertoire_command_fetches_tmdb_data_when_api_key_is_valid(
    monkeypatch: pytest.MonkeyPatch,
    settings: Settings,
    runner: CliRunner,
    venue: CinemaVenue,
    repertoire: list[Repertoire],
) -> None:
    fake_console = MagicMock()
    fake_db_manager = MagicMock()
    fake_db_manager.find_venues_by_name.return_value = [venue]
    fake_cinema = MagicMock()
    fake_cinema.fetch_repertoire = AsyncMock(return_value=repertoire)
    ratings = {"Test Movie": TmdbMovieDetails(rating="8.5/10", summary="A tense mystery.")}
    tmdb_mock = MagicMock(return_value=ratings)
    rendered = {}

    def fake_repertoire_to_cli(
        fetched_repertoire, table_metadata, ratings_payload, console
    ) -> None:
        rendered["repertoire"] = fetched_repertoire
        rendered["table_metadata"] = table_metadata
        rendered["ratings"] = ratings_payload
        rendered["console"] = console

    monkeypatch.setattr(tested_module, "_build_database_manager", lambda *_: fake_db_manager)
    monkeypatch.setattr(
        tested_module, "get_registered_chain", lambda *_: build_registered_chain(fake_cinema)
    )
    monkeypatch.setattr(tested_module, "Console", lambda: fake_console)
    monkeypatch.setattr(tested_module, "get_movie_ratings_and_summaries", tmdb_mock)
    monkeypatch.setattr(tested_module, "repertoire_to_cli", fake_repertoire_to_cli)

    app = tested_module.make_app(settings)
    result = runner.invoke(app, ["repertoire", "--chain", "cinema-city", "wroclavia", "2021-12-31"])

    assert result.exit_code == 0
    tmdb_mock.assert_called_once_with(["Test Movie"], settings.USER_PREFERENCES.TMDB_ACCESS_TOKEN)
    assert rendered["repertoire"] == repertoire
    assert rendered["ratings"] == ratings
    assert rendered["table_metadata"].chain_display_name == "Cinema City"
    assert rendered["table_metadata"].repertoire_date == "2021-12-31"
    assert rendered["console"] is fake_console


@pytest.mark.unit
def test_repertoire_command_uses_chain_default_venue_when_name_not_provided(
    monkeypatch: pytest.MonkeyPatch,
    settings: Settings,
    runner: CliRunner,
    venue: CinemaVenue,
    repertoire: list[Repertoire],
) -> None:
    fake_db_manager = MagicMock()
    fake_db_manager.find_venues_by_name.return_value = [venue]
    fake_cinema = MagicMock()
    fake_cinema.fetch_repertoire = AsyncMock(return_value=repertoire)
    monkeypatch.setattr(tested_module, "_build_database_manager", lambda *_: fake_db_manager)
    monkeypatch.setattr(
        tested_module, "get_registered_chain", lambda *_: build_registered_chain(fake_cinema)
    )
    monkeypatch.setattr(tested_module, "Console", lambda: MagicMock())
    monkeypatch.setattr(tested_module, "get_movie_ratings_and_summaries", lambda *_: {})
    monkeypatch.setattr(tested_module, "repertoire_to_cli", lambda *_: None)

    app = tested_module.make_app(settings)
    result = runner.invoke(app, ["repertoire", "--chain", "cinema-city"])

    assert result.exit_code == 0
    fake_db_manager.find_venues_by_name.assert_called_once_with(
        CinemaChainId.CINEMA_CITY, "%Wroclaw%Wroclavia%"
    )


@pytest.mark.unit
def test_repertoire_command_fails_when_chain_default_venue_is_missing(
    monkeypatch: pytest.MonkeyPatch, settings: Settings, runner: CliRunner
) -> None:
    monkeypatch.setattr(tested_module, "_build_database_manager", lambda *_: MagicMock())
    monkeypatch.setattr(
        tested_module,
        "get_registered_chain",
        lambda *_: build_registered_chain(MagicMock(), default_venue_name=None),
    )
    monkeypatch.setattr(tested_module, "Console", lambda: MagicMock())

    app = tested_module.make_app(settings)
    result = runner.invoke(app, ["repertoire", "--chain", "cinema-city"])

    assert result.exit_code == 1
    assert "Brak domyślnego lokalu skonfigurowanego dla sieci Cinema City." in result.stdout


@pytest.mark.unit
def test_repertoire_command_warns_when_tmdb_is_disabled(
    monkeypatch: pytest.MonkeyPatch,
    settings: Settings,
    runner: CliRunner,
    venue: CinemaVenue,
    repertoire: list[Repertoire],
) -> None:
    fake_console = MagicMock()
    fake_db_manager = MagicMock()
    fake_db_manager.find_venues_by_name.return_value = [venue]
    fake_cinema = MagicMock()
    fake_cinema.fetch_repertoire = AsyncMock(return_value=repertoire)
    tmdb_mock = MagicMock()
    rendered = {}

    def fake_repertoire_to_cli(
        fetched_repertoire, table_metadata, ratings_payload, console
    ) -> None:
        rendered["repertoire"] = fetched_repertoire
        rendered["ratings"] = ratings_payload

    settings.USER_PREFERENCES.TMDB_ACCESS_TOKEN = None
    monkeypatch.setattr(tested_module, "_build_database_manager", lambda *_: fake_db_manager)
    monkeypatch.setattr(
        tested_module, "get_registered_chain", lambda *_: build_registered_chain(fake_cinema)
    )
    monkeypatch.setattr(tested_module, "Console", lambda: fake_console)
    monkeypatch.setattr(tested_module, "get_movie_ratings_and_summaries", tmdb_mock)
    monkeypatch.setattr(tested_module, "repertoire_to_cli", fake_repertoire_to_cli)

    app = tested_module.make_app(settings)
    result = runner.invoke(app, ["repertoire", "--chain", "cinema-city", "wroclavia"])

    assert result.exit_code == 0
    fake_console.print.assert_called_once()
    tmdb_mock.assert_not_called()
    assert rendered["repertoire"] == repertoire
    assert rendered["ratings"] == {}


@pytest.mark.unit
def test_repertoire_command_warns_when_tmdb_lookup_fails(
    monkeypatch: pytest.MonkeyPatch,
    settings: Settings,
    runner: CliRunner,
    venue: CinemaVenue,
    repertoire: list[Repertoire],
) -> None:
    fake_console = MagicMock()
    fake_db_manager = MagicMock()
    fake_db_manager.find_venues_by_name.return_value = [venue]
    fake_cinema = MagicMock()
    fake_cinema.fetch_repertoire = AsyncMock(return_value=repertoire)
    request = httpx.Request("GET", "https://api.themoviedb.org/3/search/movie")
    response = httpx.Response(status_code=HTTPStatus.UNAUTHORIZED, request=request)
    tmdb_mock = MagicMock(
        side_effect=httpx.HTTPStatusError(
            "TMDB request failed.", request=request, response=response
        )
    )
    rendered = {}

    def fake_repertoire_to_cli(
        fetched_repertoire, table_metadata, ratings_payload, console
    ) -> None:
        rendered["repertoire"] = fetched_repertoire
        rendered["ratings"] = ratings_payload

    monkeypatch.setattr(tested_module, "_build_database_manager", lambda *_: fake_db_manager)
    monkeypatch.setattr(
        tested_module, "get_registered_chain", lambda *_: build_registered_chain(fake_cinema)
    )
    monkeypatch.setattr(tested_module, "Console", lambda: fake_console)
    monkeypatch.setattr(tested_module, "get_movie_ratings_and_summaries", tmdb_mock)
    monkeypatch.setattr(tested_module, "repertoire_to_cli", fake_repertoire_to_cli)

    app = tested_module.make_app(settings)
    result = runner.invoke(app, ["repertoire", "--chain", "cinema-city", "wroclavia"])

    assert result.exit_code == 0
    fake_console.print.assert_called_once()
    tmdb_mock.assert_called_once_with(["Test Movie"], settings.USER_PREFERENCES.TMDB_ACCESS_TOKEN)
    assert rendered["repertoire"] == repertoire
    assert rendered["ratings"] == {}


@pytest.mark.unit
def test_main_invokes_created_typer_app(monkeypatch: pytest.MonkeyPatch) -> None:
    app = MagicMock()
    make_app_mock = MagicMock(return_value=app)
    monkeypatch.setattr(tested_module, "make_app", make_app_mock)

    tested_module.main()

    make_app_mock.assert_called_once_with()
    app.assert_called_once_with()
