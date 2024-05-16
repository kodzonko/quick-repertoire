import typer
from pydantic_core import Url


def fill_string_template(text: str | Url, **kwargs) -> str:
    """Verify that all variables in string are matched in the variables dictionary.

    Args:
        text: A string to parse.
        kwargs: Variables to format the string.

    Returns:
        True if all variables are present, False otherwise.

    Raises:
        SettingsLoadError: If some variables are missing.
    """
    try:
        return str(text).format(**kwargs)
    except IndexError:  # means no placeholders to substitute
        return str(text)
    except KeyError as e:  # means some variables are missing
        typer.echo(f"Nie udało się wypełnić templatki z adresem url. Brakująca zmienna: {e}.")
        raise typer.Exit(code=1)
