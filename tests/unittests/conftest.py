import pytest


@pytest.fixture(autouse=True)
def unstub() -> None:
    from mockito import unstub

    yield
    unstub()
