from datetime import date, datetime, timedelta


def _validate_date_format(date_txt: str) -> str:
    """
    Verify that the date conforms to a required format and return it (as a string) or raise an error.

    raises:
        ValueError: if the date does not conform to the required format.
    """
    try:
        return datetime.strptime(date_txt, "%d.%m.%Y").strftime("%d.%m.%Y")
    except ValueError:
        raise ValueError("Niepoprawny format daty (dd.mm.rrrr lub dzisiaj, jutro, pojutrze)")


def date_converter(date_txt: str) -> str:
    match date_txt:
        case _validate_date_str(date_txt):
            return date_txt
        case "today" | "dzisiaj":
            return date.today().strftime("%d.%m.%Y")
        case "tomorrow" | "jutro":
            return (date.today() + timedelta(days=1)).strftime("%d.%m.%Y")
        case "pojutrze":
            return (date.today() + timedelta(days=2)).strftime("%d.%m.%Y")
        case _:
            raise ValueError("Niepoprawny format daty (dd.mm.rrrr lub dzisiaj, jutro, pojutrze)")
