from pathlib import Path
from unittest.mock import MagicMock

import pytest

import cinema_repertoire_analyzer.configuration as tested_module
from cinema_repertoire_analyzer.cinema_api.models import CinemaChainId, CinemaVenue
from cinema_repertoire_analyzer.database.database_manager import DatabaseManager
from cinema_repertoire_analyzer.exceptions import ConfigurationError
from cinema_repertoire_analyzer.settings import Settings


class StubPromptAdapter:
    """Prompt adapter returning pre-seeded answers for wizard steps."""

    def __init__(self, selections: list[object], texts: list[str], call_order: list[str]) -> None:
        self._selections = selections
        self._texts = texts
        self._call_order = call_order

    def select(self, message, choices, default=None):  # noqa: ANN001
        self._call_order.append(message)
        return self._selections.pop(0)

    def text(self, message, default=""):  # noqa: ANN001
        self._call_order.append(message)
        return self._texts.pop(0)


def _write_config_file(config_path: Path, db_file_value: str) -> None:
    config_path.write_text(
        "\n".join(
            [
                "[app]",
                f"db_file = {db_file_value}",
                "loguru_level = INFO",
                "",
                "[user_preferences]",
                "default_chain = cinema-city",
                "default_day = today",
                "tmdb_access_token =",
                "",
                "[default_venues]",
                "cinema_city = Wroclaw - Wroclavia",
                "",
                "[cinema_chains.cinema_city]",
                "repertoire_url = https://www.cinema-city.pl/#/buy-tickets-by-cinema?in-cinema={cinema_venue_id}&at={repertoire_date}",
                "venues_list_url = https://www.cinema-city.pl/#/buy-tickets-by-cinema",
                "",
            ]
        ),
        encoding="utf-8",
    )


@pytest.fixture(autouse=True)
def clear_settings_cache() -> None:
    tested_module.load_settings.cache_clear()


@pytest.mark.unit
def test_load_settings_roundtrips_config_ini(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(tested_module, "PROJECT_ROOT", tmp_path)
    expected_settings = Settings.default(project_root=tmp_path)
    expected_settings.user_preferences.tmdb_access_token = "1234"
    tested_module._write_settings(expected_settings)

    loaded_settings = tested_module.load_settings()

    assert loaded_settings == expected_settings


@pytest.mark.unit
def test_load_settings_rejects_absolute_db_paths(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(tested_module, "PROJECT_ROOT", tmp_path)
    _write_config_file(tmp_path / "config.ini", "C:/absolute/test.sqlite")

    with pytest.raises(ConfigurationError, match="względna"):
        tested_module.load_settings()


@pytest.mark.unit
def test_ensure_settings_for_argv_runs_interactive_configuration_when_config_is_missing(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(tested_module, "PROJECT_ROOT", tmp_path)
    configured_settings = Settings.default(project_root=tmp_path)
    run_interactive_configuration_mock = MagicMock(return_value=configured_settings)
    monkeypatch.setattr(
        tested_module, "run_interactive_configuration", run_interactive_configuration_mock
    )

    returned_settings = tested_module.ensure_settings_for_argv(["repertoire"])

    assert returned_settings == configured_settings
    run_interactive_configuration_mock.assert_called_once_with()


@pytest.mark.unit
def test_should_skip_bootstrap_for_help_and_completion_argv(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    assert tested_module.should_skip_bootstrap_for_argv(["--help"]) is True
    assert tested_module.should_skip_bootstrap_for_argv(["venues", "list", "--help"]) is True

    monkeypatch.setenv("APP_COMPLETE", "source_zsh")
    assert tested_module.should_skip_bootstrap_for_argv(["repertoire"]) is True


@pytest.mark.unit
def test_run_interactive_configuration_persists_selected_settings_and_venues(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    call_order: list[str] = []
    monkeypatch.setattr(tested_module, "PROJECT_ROOT", tmp_path)
    venues = [
        CinemaVenue(chain_id="cinema-city", venue_name="Warszawa - Janki", venue_id="2"),
        CinemaVenue(chain_id="cinema-city", venue_name="Wroclaw - Wroclavia", venue_id="3"),
    ]

    async def fake_fetch_all_registered_venues(
        settings: Settings, console
    ) -> dict[CinemaChainId, list[CinemaVenue]]:  # noqa: ANN001
        call_order.append("fetch")
        return {CinemaChainId.CINEMA_CITY: venues}

    prompt = StubPromptAdapter(
        selections=["INFO", "db.sqlite", CinemaChainId.CINEMA_CITY, "today", "Wroclaw - Wroclavia"],
        texts=["tmdb-token"],
        call_order=call_order,
    )
    monkeypatch.setattr(tested_module, "build_prompt_adapter", lambda: prompt)
    monkeypatch.setattr(
        tested_module, "_fetch_all_registered_venues", fake_fetch_all_registered_venues
    )

    configured_settings = tested_module.run_interactive_configuration()

    assert configured_settings.db_file == tmp_path / "db.sqlite"
    assert configured_settings.user_preferences.default_chain == CinemaChainId.CINEMA_CITY
    assert configured_settings.user_preferences.default_day == "today"
    assert configured_settings.user_preferences.tmdb_access_token == "tmdb-token"
    assert configured_settings.get_default_venue(CinemaChainId.CINEMA_CITY) == "Wroclaw - Wroclavia"
    assert (tmp_path / "config.ini").exists() is True
    assert call_order == [
        "Wybierz domyślny poziom logowania:",
        "Wybierz lokalizację pliku bazy danych:",
        "fetch",
        "Wybierz domyślną sieć kin:",
        "Wybierz domyślną datę repertuaru:",
        "Wybierz domyślny lokal:",
        "Podaj token API TMDB (pozostaw puste, aby wyłączyć TMDB):",
    ]
    persisted_db_manager = DatabaseManager(configured_settings.db_file)
    try:
        assert [
            (venue.venue_name, venue.venue_id)
            for venue in persisted_db_manager.get_all_venues(CinemaChainId.CINEMA_CITY)
        ] == [("Warszawa - Janki", "2"), ("Wroclaw - Wroclavia", "3")]
    finally:
        persisted_db_manager.close()


@pytest.mark.unit
def test_run_interactive_configuration_does_not_create_config_when_venue_fetch_fails(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(tested_module, "PROJECT_ROOT", tmp_path)
    prompt = StubPromptAdapter(selections=["INFO", "db.sqlite"], texts=[], call_order=[])

    async def fake_fetch_all_registered_venues(
        settings: Settings, console
    ) -> dict[CinemaChainId, list[CinemaVenue]]:  # noqa: ANN001
        raise ConfigurationError("boom")

    monkeypatch.setattr(tested_module, "build_prompt_adapter", lambda: prompt)
    monkeypatch.setattr(
        tested_module, "_fetch_all_registered_venues", fake_fetch_all_registered_venues
    )

    with pytest.raises(ConfigurationError, match="boom"):
        tested_module.run_interactive_configuration()

    assert (tmp_path / "config.ini").exists() is False
