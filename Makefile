PROJECT_ROOT := $(dir $(abspath $(firstword $(MAKEFILE_LIST))))
SRC := $(PROJECT_ROOT)src
TESTS := $(PROJECT_ROOT)tests
CONFIG_FILE := $(PROJECT_ROOT)pyproject.toml
.PHONY: install format lint mypy test-e2e test-int test-unit tests update

install:
	poetry install --no-interaction
	poetry run playwright install

format:
	poetry run ruff format $(SRC) $(TESTS)

lint:
	poetry run vulture $(SRC) $(TESTS) .vulture_whitelist.py
	poetry run ruff check --fix $(SRC) $(TESTS)

mypy:
	poetry run mypy $(SRC) $(TESTS)

test-e2e:
	poetry run pytest --failed-first --cov=$(SRC) -m e2e

test-int:
	poetry run pytest --failed-first --cov=$(SRC) -m integration

test-unit:
	poetry run pytest --failed-first --cov=$(SRC) -m unit

tests:
	poetry run pytest --failed-first --cov=$(SRC) $(TESTS)

update:
	poetry update
