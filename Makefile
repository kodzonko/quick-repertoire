PROJ_DIR=$(dir $(abspath $(lastword $(MAKEFILE_LIST))))
SRC=$(PROJ_DIR)src
TESTS=$(PROJ_DIR)tests

run:
	poetry run python $(SRC)/main.py

test-unit:
	poetry run pytest --failed-first --new-first --cov=$(SRC) $(TESTS)/unittests

test-int:
	poetry run pytest $(TESTS)/integration

install:
	poetry install

update:
	poetry update

lint:
	poetry run black $(SRC)
	poetry run black $(TESTS)
	poetry run isort $(SRC)
	poetry run isort $(TESTS)
	poetry run pautoflake $(SRC)
	poetry run pautoflake $(TESTS)
	poetry run flake8 --toml-config $(PROJ_DIR)pyproject.toml $(SRC)
	poetry run flake8 --toml-config $(PROJ_DIR)pyproject.toml $(TESTS)
	poetry run bandit -r $(SRC)
