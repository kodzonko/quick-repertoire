use clap::{ArgAction, Parser, Subcommand};

#[derive(Debug, Parser, Clone)]
#[command(name = "app")]
#[command(disable_help_subcommand = true)]
pub struct Cli {
    #[arg(long, action = ArgAction::SetTrue)]
    pub install_completion: bool,
    #[arg(long, action = ArgAction::SetTrue)]
    pub show_completion: bool,
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand, Clone)]
pub enum Commands {
    Configure,
    Repertoire {
        venue_name: Option<String>,
        date: Option<String>,
        #[arg(long)]
        chain: Option<String>,
    },
    Venues {
        #[command(subcommand)]
        command: VenueCommands,
    },
}

#[derive(Debug, Subcommand, Clone)]
pub enum VenueCommands {
    List {
        #[arg(long)]
        chain: Option<String>,
    },
    Update {
        #[arg(long)]
        chain: Option<String>,
    },
    Search {
        venue_name: Option<String>,
        #[arg(long)]
        chain: Option<String>,
    },
}
