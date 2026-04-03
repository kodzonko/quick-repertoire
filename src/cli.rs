use clap::{ArgAction, Parser, Subcommand};

const POLISH_HELP_TEMPLATE: &str = "\
{before-help}{name}
{about-with-newline}Użycie:
  {usage}

{all-args}{after-help}";

#[derive(Debug, Parser, Clone)]
#[command(
    name = "quickrep",
    version,
    about = "Terminalowe narzędzie do sprawdzania repertuarów kin",
    long_about = None,
    disable_help_subcommand = true,
    help_template = POLISH_HELP_TEMPLATE,
    subcommand_help_heading = "Polecenia",
    next_help_heading = "Opcje"
)]
pub struct Cli {
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Instaluje autouzupełnianie dla bieżącej powłoki",
        help_heading = "Opcje"
    )]
    pub install_completion: bool,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Wypisuje skrypt autouzupełniania dla bieżącej powłoki",
        help_heading = "Opcje"
    )]
    pub show_completion: bool,
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand, Clone)]
pub enum Commands {
    #[command(
        about = "Uruchamia konfigurację interaktywną",
        long_about = None,
        help_template = POLISH_HELP_TEMPLATE
    )]
    Configure,
    #[command(
        about = "Wyświetla obsługiwane sieci kin",
        long_about = None,
        help_template = POLISH_HELP_TEMPLATE
    )]
    Chains,
    #[command(
        about = "Wyświetla repertuar dla wybranego lokalu",
        long_about = None,
        help_template = POLISH_HELP_TEMPLATE,
        next_help_heading = "Argumenty"
    )]
    Repertoire {
        #[arg(value_name = "LOKAL", help = "Nazwa lokalu kina", help_heading = "Argumenty")]
        venue_name: Option<String>,
        #[arg(
            value_name = "DATA",
            help = "Data repertuaru: dziś, jutro lub YYYY-MM-DD",
            help_heading = "Argumenty"
        )]
        date: Option<String>,
        #[arg(long, value_name = "SIEĆ", help = "Identyfikator sieci kin", help_heading = "Opcje")]
        chain: Option<String>,
    },
    #[command(
        about = "Operacje na zapisanych lokalach kin",
        long_about = None,
        help_template = POLISH_HELP_TEMPLATE,
        subcommand_help_heading = "Polecenia"
    )]
    Venues {
        #[command(subcommand)]
        command: VenueCommands,
    },
}

#[derive(Debug, Subcommand, Clone)]
pub enum VenueCommands {
    #[command(
        about = "Wyświetla wszystkie zapisane lokale wybranej sieci",
        long_about = None,
        help_template = POLISH_HELP_TEMPLATE
    )]
    List {
        #[arg(long, value_name = "SIEĆ", help = "Identyfikator sieci kin", help_heading = "Opcje")]
        chain: Option<String>,
    },
    #[command(
        about = "Pobiera aktualną listę lokali wybranej sieci lub wszystkich obsługiwanych sieci",
        long_about = None,
        help_template = POLISH_HELP_TEMPLATE
    )]
    Update {
        #[arg(long, value_name = "SIEĆ", help = "Identyfikator sieci kin", help_heading = "Opcje")]
        chain: Option<String>,
    },
    #[command(
        about = "Wyszukuje lokale po fragmencie nazwy",
        long_about = None,
        help_template = POLISH_HELP_TEMPLATE,
        next_help_heading = "Argumenty"
    )]
    Search {
        #[arg(value_name = "LOKAL", help = "Fragment nazwy lokalu", help_heading = "Argumenty")]
        venue_name: Option<String>,
        #[arg(long, value_name = "SIEĆ", help = "Identyfikator sieci kin", help_heading = "Opcje")]
        chain: Option<String>,
    },
}
