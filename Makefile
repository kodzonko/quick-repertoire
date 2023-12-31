PROJECT_ROOT := $(dir $(abspath $(firstword $(MAKEFILE_LIST))))
SRC := $(PROJECT_ROOT)src
TESTS := $(PROJECT_ROOT)tests

install:
	poetry install

format:
	poetry run black --config $(PROJECT_ROOT)pyproject.toml $(SRC) $(TESTS)
	poetry run isort --settings-path $(PROJECT_ROOT)pyproject.toml $(SRC) $(TESTS)
	poetry run pautoflake --recursive --in-place --expand-star-imports --remove-all-unused-imports --ignore-init-module-imports $(SRC) $(TESTS)

lint:
	poetry run ruff $(SRC)

run:
	poetry run python $(SRC)/cinema_repertoire_analyzer/main.py

test-int:
	poetry run pytest $(TESTS)/integration

test-unit:
	poetry run pytest --failed-first --new-first --cov=$(SRC) $(TESTS)/unit

update:
	poetry update
