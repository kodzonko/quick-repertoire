from typing import Any

from loguru import logger

from exceptions import SettingsLoadError


def fill_string_template(text: str, variables: dict[str, Any]) -> str:
    """
    Verify that all variables in string are matched in the variables dictionary.

    Args:
        text: A string to parse.
        variables: A dictionary with variables to format the string.
    Returns:
        True if all variables are present, False otherwise.
    """
    try:
        return text.format(**variables)
    except IndexError:  # means no placeholders to substitute
        logger.info(
            "No placeholders to substitute in the url template. Returning unchanged."
        )
        return text
    except KeyError as e:  # means some variables are missing
        raise SettingsLoadError(
            "Unable to fill url template to make a request. Missing variable: %s." % e
        )
