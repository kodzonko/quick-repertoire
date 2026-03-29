class AppError(Exception):
    """Base application error with a user-facing message."""


class DatabaseConnectionError(AppError):
    """Raised when the app cannot connect to the database."""


class VenueNotFoundError(AppError):
    """Raised when no venue matches the provided query."""


class AmbiguousVenueMatchError(AppError):
    """Raised when multiple venues match a query that needs a single result."""

    def __init__(self, matches_count: int) -> None:
        noun = "pasujące wyniki" if matches_count < 5 else "pasujących wyników"
        super().__init__(
            f"Podana nazwa lokalu jest niejednoznaczna. Znaleziono {matches_count} {noun}."
        )
        self.matches_count = matches_count


class UnsupportedCinemaChainError(AppError):
    """Raised when the requested cinema chain is not registered."""

    def __init__(self, invalid_chain: str, supported_chains: str) -> None:
        super().__init__(
            f"Nieobsługiwana sieć kin: {invalid_chain}. Dostępne wartości: {supported_chains}."
        )
        self.invalid_chain = invalid_chain
        self.supported_chains = supported_chains


class DefaultVenueNotConfiguredError(AppError):
    """Raised when no default venue exists for a selected cinema chain."""

    def __init__(self, chain_display_name: str) -> None:
        super().__init__(f"Brak domyślnego lokalu skonfigurowanego dla sieci {chain_display_name}.")
        self.chain_display_name = chain_display_name


class ConfigurationNotFoundError(AppError):
    """Raised when the runtime config file is missing."""


class ConfigurationError(AppError):
    """Raised when the runtime config cannot be loaded or created."""


class ConfigurationAbortedError(AppError):
    """Raised when the user aborts interactive configuration."""


class TemplateRenderError(AppError):
    """Raised when a URL template cannot be filled."""

    def __init__(self, missing_variable: str) -> None:
        super().__init__(
            "Nie udało się wypełnić templatki z adresem url. "
            f"Brakująca zmienna: {missing_variable}."
        )
        self.missing_variable = missing_variable
