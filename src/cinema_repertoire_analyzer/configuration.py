import configparser
import os
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path
from tempfile import NamedTemporaryFile
from typing import Protocol, TypeVar

import anyio
import questionary
from pydantic import ValidationError
from rich.console import Console
from rich.progress import BarColumn, Progress, SpinnerColumn, TaskProgressColumn, TextColumn

from cinema_repertoire_analyzer.cinema_api.models import CinemaChainId, CinemaVenue
from cinema_repertoire_analyzer.cinema_api.registry import (
    RegisteredCinemaChain,
    get_registered_chains,
)
from cinema_repertoire_analyzer.database.database_manager import DatabaseManager
from cinema_repertoire_analyzer.exceptions import (
    ConfigurationAbortedError,
    ConfigurationError,
    ConfigurationNotFoundError,
)
from cinema_repertoire_analyzer.settings import LOG_LVLS, PROJECT_ROOT, AllowedDefaultDays, Settings

CONFIG_FILE_NAME = "config.ini"
DB_FILE_CHOICES = ("db.sqlite", "data/db.sqlite")
DEFAULT_DAY_CHOICES = ("today", "tomorrow")
LOG_LEVEL_CHOICES: tuple[LOG_LVLS, ...] = ("INFO", "DEBUG", "WARNING", "ERROR", "CRITICAL", "TRACE")
HELP_AND_COMPLETION_FLAGS = {"-h", "--help", "--install-completion", "--show-completion"}

PromptValue = TypeVar("PromptValue")


@dataclass(frozen=True)
class SelectionChoice[PromptValue]:
    """A single selectable prompt choice."""

    title: str
    value: PromptValue


class PromptAdapter(Protocol):
    """Interactive prompt adapter used by the configuration wizard."""

    def select(
        self,
        message: str,
        choices: list[SelectionChoice[PromptValue]],
        default: PromptValue | None = None,
    ) -> PromptValue:
        """Select a value from an arrow-key list."""

    def text(self, message: str, default: str = "") -> str:
        """Read a single-line text response."""


class QuestionaryPromptAdapter:
    """Questionary-backed implementation of the interactive prompt adapter."""

    def select(
        self,
        message: str,
        choices: list[SelectionChoice[PromptValue]],
        default: PromptValue | None = None,
    ) -> PromptValue:
        """Select a value from an interactive arrow-key list."""
        questionary_choices = [
            questionary.Choice(title=choice.title, value=choice.value) for choice in choices
        ]
        default_choice = next(
            (choice for choice in questionary_choices if choice.value == default), None
        )
        answer = questionary.select(
            message=message,
            choices=questionary_choices,
            default=default_choice,
            use_arrow_keys=True,
        ).ask()
        if answer is None:
            raise ConfigurationAbortedError("Konfiguracja została przerwana przez użytkownika.")
        return answer

    def text(self, message: str, default: str = "") -> str:
        """Read a single-line text response from the user."""
        answer = questionary.text(message=message, default=default).ask()
        if answer is None:
            raise ConfigurationAbortedError("Konfiguracja została przerwana przez użytkownika.")
        return answer


def build_prompt_adapter() -> PromptAdapter:
    """Create the interactive prompt adapter used for configuration."""
    return QuestionaryPromptAdapter()


def _config_file_path() -> Path:
    return PROJECT_ROOT / CONFIG_FILE_NAME


def _should_skip_bootstrap_for_argv(argv: list[str]) -> bool:
    if any(argument in HELP_AND_COMPLETION_FLAGS for argument in argv):
        return True
    return any(environment_key.endswith("_COMPLETE") for environment_key in os.environ)


def _is_configure_command(argv: list[str]) -> bool:
    for argument in argv:
        if argument.startswith("-"):
            continue
        return argument == "configure"
    return False


def _default_chain_section_name(chain_id: CinemaChainId) -> str:
    return chain_id.value.replace("-", "_")


def _cinema_chain_section_name(chain_id: CinemaChainId) -> str:
    return f"cinema_chains.{_default_chain_section_name(chain_id)}"


def _relative_db_file_path(db_file: Path) -> Path:
    if db_file.is_absolute():
        try:
            return db_file.relative_to(PROJECT_ROOT)
        except ValueError as error:
            raise ConfigurationError(
                "Ścieżka pliku bazy danych musi wskazywać lokalizację wewnątrz katalogu projektu."
            ) from error
    return db_file


def _resolve_db_file_path(raw_db_file_path: str) -> Path:
    stripped_path = raw_db_file_path.strip()
    if not stripped_path:
        raise ConfigurationError("W config.ini brakuje wartosci app.db_file.")
    db_file_path = Path(stripped_path)
    if db_file_path.is_absolute():
        raise ConfigurationError(
            "Ścieżka pliku bazy danych w config.ini musi być względna wobec katalogu projektu."
        )
    return PROJECT_ROOT / db_file_path


def _config_parser_to_settings(config: configparser.ConfigParser) -> Settings:
    try:
        default_venues_data = {
            _default_chain_section_name(chain.chain_id): config.get(
                "default_venues", _default_chain_section_name(chain.chain_id)
            )
            for chain in get_registered_chains()
        }
        cinema_chains_data = {
            _default_chain_section_name(chain.chain_id): {
                "repertoire_url": config.get(
                    _cinema_chain_section_name(chain.chain_id), "repertoire_url"
                ),
                "venues_list_url": config.get(
                    _cinema_chain_section_name(chain.chain_id), "venues_list_url"
                ),
            }
            for chain in get_registered_chains()
        }
        return Settings.model_validate(
            {
                "db_file": _resolve_db_file_path(config.get("app", "db_file")),
                "loguru_level": config.get("app", "loguru_level"),
                "user_preferences": {
                    "default_chain": CinemaChainId.from_value(
                        config.get("user_preferences", "default_chain")
                    ),
                    "default_day": config.get("user_preferences", "default_day"),
                    "tmdb_access_token": config.get("user_preferences", "tmdb_access_token"),
                    "default_venues": default_venues_data,
                },
                "cinema_chains": cinema_chains_data,
            }
        )
    except (configparser.Error, ValidationError, ValueError, ConfigurationError) as error:
        if isinstance(error, ConfigurationError):
            raise
        raise ConfigurationError(
            "Nie udało się wczytać config.ini. Uruchom `app configure`, aby odtworzyć konfigurację."
        ) from error


def _settings_to_config_parser(settings: Settings) -> configparser.ConfigParser:
    config = configparser.ConfigParser()
    config["app"] = {
        "db_file": _relative_db_file_path(settings.db_file).as_posix(),
        "loguru_level": settings.loguru_level,
    }
    config["user_preferences"] = {
        "default_chain": settings.user_preferences.default_chain.value,
        "default_day": settings.user_preferences.default_day,
        "tmdb_access_token": settings.user_preferences.tmdb_access_token or "",
    }
    config["default_venues"] = {
        _default_chain_section_name(chain.chain_id): settings.get_default_venue(chain.chain_id)
        or ""
        for chain in get_registered_chains()
    }
    for chain in get_registered_chains():
        chain_settings = settings.cinema_chains.get(chain.chain_id)
        config[_cinema_chain_section_name(chain.chain_id)] = {
            "repertoire_url": chain_settings.repertoire_url,
            "venues_list_url": chain_settings.venues_list_url,
        }
    return config


def _write_settings(settings: Settings) -> None:
    config_path = _config_file_path()
    config_path.parent.mkdir(parents=True, exist_ok=True)
    parser = _settings_to_config_parser(settings)
    with NamedTemporaryFile(
        "w", encoding="utf-8", dir=config_path.parent, delete=False, newline="\n"
    ) as temp_file:
        parser.write(temp_file)
        temp_file_path = Path(temp_file.name)
    temp_file_path.replace(config_path)
    load_settings.cache_clear()


@lru_cache
def load_settings() -> Settings:
    """Load runtime settings from config.ini in the project root."""
    config_path = _config_file_path()
    if not config_path.exists():
        raise ConfigurationNotFoundError(
            "Nie znaleziono pliku konfiguracji: "
            f"{config_path.name}. Uruchom aplikację ponownie, aby przejść "
            "przez konfigurację początkową."
        )

    parser = configparser.ConfigParser()
    read_files = parser.read(config_path, encoding="utf-8")
    if not read_files:
        raise ConfigurationError("Nie udało się odczytać pliku config.ini.")
    return _config_parser_to_settings(parser)


def _build_working_settings(
    base_settings: Settings, *, loguru_level: LOG_LVLS, db_file_choice: str
) -> Settings:
    settings_data = base_settings.model_dump(mode="python")
    settings_data["loguru_level"] = loguru_level
    settings_data["db_file"] = PROJECT_ROOT / db_file_choice
    return Settings.model_validate(settings_data)


async def _fetch_all_registered_venues(
    settings: Settings, console: Console
) -> dict[CinemaChainId, list[CinemaVenue]]:
    chains = tuple(get_registered_chains())
    if not chains:
        raise ConfigurationError("Brak zarejestrowanych sieci kin do skonfigurowania.")

    progress = Progress(
        SpinnerColumn(),
        TextColumn("[progress.description]{task.description}"),
        BarColumn(),
        TaskProgressColumn(),
        console=console,
        transient=False,
    )
    overall_task = progress.add_task(
        "Pobieranie list lokali dla wszystkich sieci", total=len(chains)
    )
    chain_tasks = {
        chain.chain_id: progress.add_task(f"{chain.display_name}: oczekiwanie", total=1)
        for chain in chains
    }
    venues_by_chain: dict[CinemaChainId, list[CinemaVenue]] = {}
    failed_chains: list[str] = []

    async def fetch_chain_venues(chain: RegisteredCinemaChain) -> None:
        task_id = chain_tasks[chain.chain_id]
        progress.update(task_id, description=f"{chain.display_name}: pobieranie")
        try:
            venues = await chain.client_factory(settings).fetch_venues()
        except Exception:
            failed_chains.append(chain.display_name)
            progress.update(task_id, completed=1, description=f"{chain.display_name}: błąd")
        else:
            if not venues:
                failed_chains.append(chain.display_name)
                progress.update(
                    task_id, completed=1, description=f"{chain.display_name}: brak lokali"
                )
                return
            venues_by_chain[chain.chain_id] = sorted(
                venues, key=lambda venue: venue.venue_name.casefold()
            )
            progress.update(
                task_id, completed=1, description=f"{chain.display_name}: {len(venues)} lokali"
            )
        finally:
            progress.advance(overall_task)

    with progress:
        async with anyio.create_task_group() as task_group:
            for chain in chains:
                task_group.start_soon(fetch_chain_venues, chain)

    if failed_chains:
        failed_chain_list = ", ".join(sorted(failed_chains))
        raise ConfigurationError(
            "Nie udało się pobrać list lokali dla wszystkich sieci. "
            f"Niepowodzenie: {failed_chain_list}."
        )
    return venues_by_chain


def _persist_venues(
    settings: Settings, venues_by_chain: dict[CinemaChainId, list[CinemaVenue]]
) -> None:
    db_manager = DatabaseManager(settings.db_file)
    try:
        db_manager.replace_venues_batch(
            {chain_id.value: venues for chain_id, venues in venues_by_chain.items()}
        )
    finally:
        db_manager.close()


def _chain_choices() -> list[SelectionChoice[CinemaChainId]]:
    return [
        SelectionChoice(title=chain.display_name, value=chain.chain_id)
        for chain in get_registered_chains()
    ]


def _default_value_if_present[ValueT](
    choices: list[SelectionChoice[ValueT]], default: ValueT | None
) -> ValueT | None:
    if default is None:
        return None
    for choice in choices:
        if choice.value == default:
            return choice.value
    return None


def run_interactive_configuration(existing_settings: Settings | None = None) -> Settings:
    """Run the first-run / reconfiguration wizard and persist config.ini."""
    base_settings = existing_settings or Settings.default(project_root=PROJECT_ROOT)
    prompt = build_prompt_adapter()
    console = Console()

    db_file_choices = [SelectionChoice(title=choice, value=choice) for choice in DB_FILE_CHOICES]
    default_db_file_choice = _default_value_if_present(
        db_file_choices, _relative_db_file_path(base_settings.db_file).as_posix()
    )
    selected_log_level = prompt.select(
        "Wybierz domyślny poziom logowania:",
        [SelectionChoice(title=choice, value=choice) for choice in LOG_LEVEL_CHOICES],
        default=base_settings.loguru_level,
    )
    selected_db_file = prompt.select(
        "Wybierz lokalizację pliku bazy danych:", db_file_choices, default=default_db_file_choice
    )
    working_settings = _build_working_settings(
        base_settings, loguru_level=selected_log_level, db_file_choice=selected_db_file
    )

    venues_by_chain = anyio.run(_fetch_all_registered_venues, working_settings, console)

    chain_choices = _chain_choices()
    selected_default_chain = prompt.select(
        "Wybierz domyślną sieć kin:",
        chain_choices,
        default=_default_value_if_present(
            chain_choices, base_settings.user_preferences.default_chain
        ),
    )
    default_day_choices: list[SelectionChoice[AllowedDefaultDays]] = [
        SelectionChoice(title="today", value="today"),
        SelectionChoice(title="tomorrow", value="tomorrow"),
    ]
    selected_default_day = prompt.select(
        "Wybierz domyślną datę repertuaru:",
        default_day_choices,
        default=base_settings.user_preferences.default_day,
    )
    selected_default_venue = prompt.select(
        "Wybierz domyślny lokal:",
        [
            SelectionChoice(title=venue.venue_name, value=venue.venue_name)
            for venue in venues_by_chain[selected_default_chain]
        ],
        default=base_settings.get_default_venue(selected_default_chain),
    )
    selected_tmdb_access_token = prompt.text(
        "Podaj token API TMDB (pozostaw puste, aby wyłączyć TMDB):",
        default=base_settings.user_preferences.tmdb_access_token or "",
    )

    working_settings.user_preferences.default_chain = selected_default_chain
    working_settings.user_preferences.default_day = selected_default_day
    working_settings.user_preferences.tmdb_access_token = selected_tmdb_access_token.strip() or None
    setattr(
        working_settings.user_preferences.default_venues,
        _default_chain_section_name(selected_default_chain),
        selected_default_venue,
    )

    _persist_venues(working_settings, venues_by_chain)
    _write_settings(working_settings)
    console.print(f"Konfiguracja zapisana w {_config_file_path().name}.", style="bold green")
    return working_settings


def ensure_settings_for_argv(argv: list[str]) -> Settings:
    """Load settings or trigger first-run configuration for executable commands."""
    try:
        return load_settings()
    except ConfigurationNotFoundError:
        return run_interactive_configuration()


def load_settings_if_available() -> Settings | None:
    """Load settings when config exists, otherwise return None."""
    try:
        return load_settings()
    except ConfigurationError, ConfigurationNotFoundError:
        return None


def should_skip_bootstrap_for_argv(argv: list[str]) -> bool:
    """Return whether startup should skip config bootstrap for the current argv."""
    return _should_skip_bootstrap_for_argv(argv)


def should_defer_bootstrap_to_command(argv: list[str]) -> bool:
    """Return whether startup should let a CLI command handle configuration."""
    return _is_configure_command(argv)
