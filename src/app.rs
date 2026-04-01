use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use clap::{CommandFactory, Parser};
use clap_complete::{Shell, generate};

use crate::cinema::cinema_city::ChromiumHtmlRenderer;
use crate::cinema::registry::{RegisteredCinemaChain, Registry};
use crate::cli::{Cli, Commands, VenueCommands};
use crate::config::{
    AppPaths, BINARY_NAME, PromptAdapter, RuntimeWriteAccessProbe, Settings, build_prompt_adapter,
    build_runtime_write_access_probe, ensure_settings_for_argv_with_write_access_probe,
    load_settings, load_settings_if_available,
    run_interactive_configuration_with_write_access_probe, should_defer_bootstrap_to_command,
    should_skip_bootstrap_for_argv,
};
use crate::domain::{
    CinemaChainId, CinemaVenue, RepertoireCliTableMetadata, TmdbLookupMovie, TmdbMovieDetails,
};
use crate::error::{AppError, AppResult};
use crate::logging::init_logging;
use crate::output::{
    StdoutTerminal, Terminal, cinema_venue_input_parser, date_input_parser,
    render_repertoire_table, render_venues_table,
};
use crate::persistence::DatabaseManager;
use crate::tmdb::{ReqwestTmdbClient, TmdbService};

pub struct AppDependencies {
    pub paths: AppPaths,
    pub prompt: Box<dyn PromptAdapter>,
    pub registry: Registry,
    pub tmdb_client: Arc<dyn TmdbService>,
    pub runtime_write_access_probe: Box<dyn RuntimeWriteAccessProbe>,
}

#[derive(Clone, Copy)]
struct CommandContext<'a> {
    settings: &'a Settings,
    paths: &'a AppPaths,
}

impl AppDependencies {
    pub fn real(paths: AppPaths) -> AppResult<Self> {
        Ok(Self {
            paths,
            prompt: build_prompt_adapter(),
            registry: Registry::new(Arc::new(ChromiumHtmlRenderer)),
            tmdb_client: Arc::new(ReqwestTmdbClient::new()?),
            runtime_write_access_probe: build_runtime_write_access_probe(),
        })
    }
}

pub async fn run_main() -> i32 {
    let paths = match AppPaths::from_current_exe() {
        Ok(paths) => paths,
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    };
    let dependencies = match AppDependencies::real(paths) {
        Ok(dependencies) => dependencies,
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    };
    let args = std::env::args().collect::<Vec<_>>();
    let mut terminal = StdoutTerminal;
    run_with_args(args, &dependencies, &mut terminal).await
}

pub async fn run_with_args(
    args: Vec<String>,
    dependencies: &AppDependencies,
    terminal: &mut dyn Terminal,
) -> i32 {
    init_logging();

    let argv = args.iter().skip(1).cloned().collect::<Vec<_>>();
    let mut settings =
        if should_skip_bootstrap_for_argv(&argv) || should_defer_bootstrap_to_command(&argv) {
            load_settings_if_available(&dependencies.paths)
        } else {
            match ensure_settings_for_argv_with_write_access_probe(
                &dependencies.paths,
                &dependencies.registry,
                dependencies.prompt.as_ref(),
                dependencies.runtime_write_access_probe.as_ref(),
            )
            .await
            {
                Ok(settings) => Some(settings),
                Err(error) => {
                    terminal.write_line(&error.to_string());
                    return 1;
                }
            }
        };

    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            let _ = error.print();
            return if error.use_stderr() { 2 } else { 0 };
        }
    };

    if cli.show_completion || cli.install_completion {
        return show_completion();
    }

    let Some(command) = cli.command else {
        let mut command = Cli::command();
        let _ = command.print_help();
        println!();
        return 0;
    };

    if matches!(command, Commands::Configure) {
        return match run_interactive_configuration_with_write_access_probe(
            &dependencies.paths,
            settings.take(),
            &dependencies.registry,
            dependencies.prompt.as_ref(),
            dependencies.runtime_write_access_probe.as_ref(),
        )
        .await
        {
            Ok(_) => {
                terminal.write_line("Konfiguracja zapisana w config.ini.");
                0
            }
            Err(error) => {
                terminal.write_line(&error.to_string());
                1
            }
        };
    }

    let settings = match settings {
        Some(settings) => settings,
        None => match load_settings(&dependencies.paths) {
            Ok(settings) => settings,
            Err(error) => {
                terminal.write_line(&error.to_string());
                return 1;
            }
        },
    };

    let result = match command {
        Commands::Configure => unreachable!(),
        Commands::Repertoire { chain, venue_name, date } => {
            handle_repertoire(
                CommandContext { settings: &settings, paths: &dependencies.paths },
                &dependencies.registry,
                dependencies.tmdb_client.as_ref(),
                chain,
                venue_name,
                date,
                terminal,
            )
            .await
        }
        Commands::Venues { command } => match command {
            VenueCommands::List { chain } => {
                handle_venues_list(
                    &settings,
                    &dependencies.paths,
                    &dependencies.registry,
                    chain,
                    terminal,
                )
                .await
            }
            VenueCommands::Update { chain } => {
                handle_venues_update(
                    &settings,
                    &dependencies.paths,
                    &dependencies.registry,
                    chain,
                    terminal,
                )
                .await
            }
            VenueCommands::Search { venue_name, chain } => {
                handle_venues_search(
                    &settings,
                    &dependencies.paths,
                    &dependencies.registry,
                    chain,
                    venue_name,
                    terminal,
                )
                .await
            }
        },
    };

    match result {
        Ok(()) => 0,
        Err(error) => {
            terminal.write_line(&error.to_string());
            1
        }
    }
}

pub fn resolve_single_venue(found_venues: &[CinemaVenue]) -> AppResult<CinemaVenue> {
    match found_venues {
        [] => Err(AppError::VenueNotFound),
        [venue] => Ok(venue.clone()),
        _ => Err(AppError::AmbiguousVenueMatch { matches_count: found_venues.len() }),
    }
}

fn resolve_chain(
    chain: Option<String>,
    settings: &Settings,
    registry: &Registry,
) -> AppResult<RegisteredCinemaChain> {
    let chain_id = match chain {
        Some(chain) => CinemaChainId::from_value(&chain)?,
        None => settings.user_preferences.default_chain,
    };
    registry.get_registered_chain(chain_id)
}

fn resolve_venue_name(
    venue_name: Option<String>,
    chain: &RegisteredCinemaChain,
    settings: &Settings,
) -> AppResult<String> {
    match venue_name {
        Some(venue_name) => Ok(venue_name),
        None => settings.get_default_venue(chain.chain_id).map(str::to_string).ok_or_else(|| {
            AppError::DefaultVenueNotConfigured { chain_display_name: chain.display_name.clone() }
        }),
    }
}

fn build_database_manager(paths: &AppPaths) -> AppResult<DatabaseManager> {
    DatabaseManager::new(paths.db_file())
}

async fn handle_repertoire(
    context: CommandContext<'_>,
    registry: &Registry,
    tmdb_client: &dyn TmdbService,
    chain: Option<String>,
    venue_name: Option<String>,
    date: Option<String>,
    terminal: &mut dyn Terminal,
) -> AppResult<()> {
    let registered_chain = resolve_chain(chain, context.settings, registry)?;
    let resolved_venue_name = resolve_venue_name(venue_name, &registered_chain, context.settings)?;
    let venue_name_parsed = cinema_venue_input_parser(&resolved_venue_name);
    let db_manager = build_database_manager(context.paths)?;
    let found_venues =
        db_manager.find_venues_by_name(registered_chain.chain_id.as_str(), &venue_name_parsed)?;
    let venue = resolve_single_venue(&found_venues)?;
    let date_parsed = date_input_parser(
        date.as_deref().unwrap_or(&context.settings.user_preferences.default_day),
    )?;
    let cinema_client = (registered_chain.client_factory)(context.settings);
    let fetched_repertoire = cinema_client.fetch_repertoire(&date_parsed, &venue).await?;
    let lookup_movies = fetched_repertoire.iter().map(TmdbLookupMovie::from).collect::<Vec<_>>();
    let ratings = load_tmdb_ratings(
        &lookup_movies,
        context.settings.user_preferences.tmdb_access_token.as_deref(),
        tmdb_client,
        terminal,
    )
    .await;

    let table_metadata = RepertoireCliTableMetadata {
        chain_display_name: registered_chain.display_name,
        repertoire_date: date_parsed,
        cinema_venue_name: venue.venue_name,
    };
    terminal.write_line(&render_repertoire_table(&fetched_repertoire, &table_metadata, &ratings));
    Ok(())
}

async fn handle_venues_list(
    settings: &Settings,
    paths: &AppPaths,
    registry: &Registry,
    chain: Option<String>,
    terminal: &mut dyn Terminal,
) -> AppResult<()> {
    let registered_chain = resolve_chain(chain, settings, registry)?;
    let db_manager = build_database_manager(paths)?;
    let venues = db_manager.get_all_venues(registered_chain.chain_id.as_str())?;
    terminal.write_line(&render_venues_table(&venues, &registered_chain.display_name));
    Ok(())
}

async fn handle_venues_update(
    settings: &Settings,
    paths: &AppPaths,
    registry: &Registry,
    chain: Option<String>,
    terminal: &mut dyn Terminal,
) -> AppResult<()> {
    let registered_chain = resolve_chain(chain, settings, registry)?;
    terminal.write_line(&format!(
        "Aktualizowanie lokali dla sieci: {}...",
        registered_chain.display_name
    ));
    let cinema_client = (registered_chain.client_factory)(settings);
    let venues = cinema_client.fetch_venues().await?;
    let db_manager = build_database_manager(paths)?;
    db_manager.replace_venues(registered_chain.chain_id.as_str(), &venues)?;
    terminal.write_line("Lokale zaktualizowane w lokalnej bazie danych.");
    Ok(())
}

async fn handle_venues_search(
    settings: &Settings,
    paths: &AppPaths,
    registry: &Registry,
    chain: Option<String>,
    venue_name: Option<String>,
    terminal: &mut dyn Terminal,
) -> AppResult<()> {
    let registered_chain = resolve_chain(chain, settings, registry)?;
    let db_manager = build_database_manager(paths)?;
    let venues = db_manager.find_venues_by_name(
        registered_chain.chain_id.as_str(),
        &cinema_venue_input_parser(venue_name.as_deref().unwrap_or_default()),
    )?;
    terminal.write_line(&render_venues_table(&venues, &registered_chain.display_name));
    Ok(())
}

async fn load_tmdb_ratings(
    lookup_movies: &[TmdbLookupMovie],
    access_token: Option<&str>,
    tmdb_client: &dyn TmdbService,
    terminal: &mut dyn Terminal,
) -> HashMap<String, TmdbMovieDetails> {
    if lookup_movies.is_empty() {
        return HashMap::new();
    }
    let Some(access_token) = access_token else {
        terminal.write_line(
            "Klucz API do usługi TMDB nie jest skonfigurowany. Niektóre funkcje mogą być niedostępne.",
        );
        return HashMap::new();
    };

    match tmdb_client.get_movie_ratings_and_summaries(lookup_movies, access_token).await {
        Ok(ratings) => ratings,
        Err(_) => {
            terminal.write_line(
                "Nie udało się pobrać danych z usługi TMDB. Niektóre funkcje mogą być niedostępne.",
            );
            HashMap::new()
        }
    }
}

fn show_completion() -> i32 {
    let shell = if cfg!(windows) {
        Some(Shell::PowerShell)
    } else {
        let shell = std::env::var("SHELL").ok();
        shell.and_then(|shell| {
            let shell_name = Path::new(&shell).file_name()?.to_string_lossy();
            match shell_name.as_ref() {
                "bash" => Some(Shell::Bash),
                "zsh" => Some(Shell::Zsh),
                "fish" => Some(Shell::Fish),
                "elvish" => Some(Shell::Elvish),
                _ => None,
            }
        })
    };

    match shell {
        Some(shell) => {
            let mut command = Cli::command();
            generate(shell, &mut command, BINARY_NAME, &mut std::io::stdout());
            0
        }
        None => {
            eprintln!("Nie udało się wykryć powłoki do generowania completion.");
            1
        }
    }
}
