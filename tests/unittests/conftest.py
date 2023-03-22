import pytest


@pytest.fixture
def unstub() -> None:
    from mockito import unstub

    yield
    unstub()
