#[tokio::main]
async fn main() {
    std::process::exit(cinema_repertoire_analyzer::app::run_main().await);
}
