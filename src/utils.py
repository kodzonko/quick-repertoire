import re
from typing import Any


def verify_string_formatting_variables_match(
        text: str, variables: dict[str, Any]
) -> bool:
    """
    Verify that all variables in string are matched in the variables dictionary.

    Args:
        text: A string to parse.
        variables: A dictionary with variables to format the string.
    Returns:
        True if all variables are present, False otherwise.
    """
    substituted = text.format(**variables)
    variables_in_text = re.findall(r"{[a-zA-Z0-9_]*}", substituted)
    import pdb

    pdb.set_trace()
    # If there are any unsubstituted variables left (list not empty), return False
    return False if variables_in_text else True
