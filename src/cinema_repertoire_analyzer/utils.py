from loguru import logger

from cinema_repertoire_analyzer.exceptions import SettingsLoadError


def fill_string_template(text: str, **kwargs) -> str:
    """
    Verify that all variables in string are matched in the variables dictionary.

    Args:
        text: A string to parse.
        kwargs: Variables to format the string.

    Returns:
        True if all variables are present, False otherwise.

    Raises:
        SettingsLoadError: If some variables are missing.
    """
    try:
        return text.format(**kwargs)
    except IndexError:  # means no placeholders to substitute
        logger.info(
            "No placeholders to substitute in the url template. Returning unchanged."
        )
        return text
    except KeyError as e:  # means some variables are missing
        raise SettingsLoadError(
            "Unable to fill url template to make a request. Missing variable: %s." % e
        )
