PROJECT_ROOT := $(dir $(abspath $(firstword $(MAKEFILE_LIST))))
SRC := $(PROJECT_ROOT)src
TESTS := $(PROJECT_ROOT)tests
CONFIG_FILE := $(PROJECT_ROOT)pyproject.toml

install:
	poetry install

format:
	poetry run black --config $(CONFIG_FILE) $(SRC) $(TESTS)
	poetry run isort --settings-path $(CONFIG_FILE) $(SRC) $(TESTS)
	poetry run pautoflake --recursive --in-place --expand-star-imports --remove-all-unused-imports --ignore-init-module-imports $(SRC) $(TESTS)

lint:
	poetry run ruff $(SRC)

run:
	poetry run python $(SRC)/cinema_repertoire_analyzer/main.py

test-e2e:
	poetry run pytest -m e2e

test-int:
	poetry run pytest -m integration

test-unit:
	poetry run pytest --failed-first --new-first --cov=$(SRC) -m unit

update:
	poetry update
