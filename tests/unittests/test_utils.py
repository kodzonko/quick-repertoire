from typing import Any

import pytest

from utils import verify_string_formatting_variables_match


@pytest.mark.parametrize(
    "text, variables, expected",
    [
        ("fizz{a}buzz", {"a": "qwerty"}, True),
        ("some{a}text{a}", {"a": "qwerty", "b": 123}, True),
        (
                "lorem {a_placeholder} dolor://{other_placeholder_11}",
                {
                    "a_placeholder": "ipsum",
                    "other_placeholder_11": "sit",
                    "redundant_var": False,
                },
                True,
        ),
        ("{a} {b} {missing_variable}", {"a": "sth", "b": "text"}, False),
        ("{} no placeholders", {"fizz": "buzz"}, True),
    ],
)
def test_verify_string_formatting_variables_match_verifies_correctly(
        text: str, variables: dict[str, Any], expected: bool
) -> None:
    assert verify_string_formatting_variables_match(text, variables) == expected
