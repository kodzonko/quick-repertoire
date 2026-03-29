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


class TemplateRenderError(AppError):
    """Raised when a URL template cannot be filled."""

    def __init__(self, missing_variable: str) -> None:
        super().__init__(
            "Nie udało się wypełnić templatki z adresem url. "
            f"Brakująca zmienna: {missing_variable}."
        )
        self.missing_variable = missing_variable
