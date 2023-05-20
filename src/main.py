from pathlib import Path

import typer

app = typer.Typer()


@app.command()
def repertoire(date: str, cinema_chain: str, cinema_venue: str) -> None:
    pass


@app.command()
def update():
    pass


if __name__ == "__main__":
    app()
