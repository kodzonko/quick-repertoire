import pytest


@pytest.fixture(autouse=True)
def unstub() -> None:  # type: ignore[misc]
    from mockito import unstub

    yield
    unstub()
