from typing import Any

import pytest

from cinema_repertoire_analyzer.cinema_api.template_utils import fill_string_template
from cinema_repertoire_analyzer.exceptions import SettingsLoadError


@pytest.mark.parametrize(
    "text, variables, expected",
    [
        ("fizz{a}buzz", {"a": "qwerty"}, "fizzqwertybuzz"),
        ("some{a}text{a}", {"a": "qwerty", "b": 123}, "someqwertytextqwerty"),
        (
            "lorem {a_placeholder} dolor://{other_placeholder_11}",
            {"a_placeholder": "ipsum", "other_placeholder_11": "sit", "redundant_var": False},
            "lorem ipsum dolor://sit",
        ),
        ("{} no placeholders", {"fizz": "buzz"}, "{} no placeholders"),
    ],
)
def test_fill_string_template_returns_correctly_filled_string(
    text: str, variables: dict[str, Any], expected: bool
) -> None:
    assert fill_string_template(text, **variables) == expected


def test_fill_string_template_raises_error_on_missing_variable() -> None:
    with pytest.raises(
        SettingsLoadError,
        match=(
            "Unable to fill url template to make a request. Missing variable: "
            "'missing_variable'."
        ),
    ):
        fill_string_template("{missing_variable}", a="sth", b="text")
