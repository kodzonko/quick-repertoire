from cinema_repertoire_analyzer.exceptions import TemplateRenderError


def fill_string_template(text: str, **kwargs) -> str:
    """Verify that all variables in string are matched in the variables dictionary.

    Args:
        text: A string to parse.
        kwargs: Variables to format the string.

    Returns:
        The formatted string.

    Raises:
        TemplateRenderError: If some variables are missing.
    """
    try:
        return str(text).format(**kwargs)
    except IndexError:  # means no placeholders to substitute
        return str(text)
    except KeyError as e:  # means some variables are missing
        raise TemplateRenderError(str(e).strip("'\"")) from e
