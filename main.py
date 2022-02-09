import argparse
import logging
from pathlib import Path

logging.basicConfig(level=logging.DEBUG, filename="app.log", filemode="w")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "cinema_chain",
        help="A name of a cinema chain to look up repertoire for",
        type=str,
        choices={"Cinema City", "Multikino", "Helios"},
    )
    parser.add_argument("cinema_venue", help="A name of venue (branch) to look up the repertoire for")
    parser.add_argument("-u", "--update", help="Update a list of known cinema venues")
    parser.add_argument("-s", "--save", help="A path to file to save data in")
    args = parser.parse_args()
    # Program main loop
    while True:
        # TODO: load settings

        # TODO: if args.update download a current list of cinemas

        # TODO: check for JSON with cinemas list if doesn't exist notify a user

        # TODO: Get the repertoire for a given cinema and day with ratings from filmweb, IMDB and metacritic

        if args.save:
            path = Path(args.save)
            logging.info(f"Writing data to a file: {path}")
            # TODO: implement

        # TODO: Ask user if check a repertoire for another date or quit the program (assume the same cinema)
        break

    print("All done.")


if __name__ == "__main__":
    main()
