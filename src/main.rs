#[tokio::main]
async fn main() {
    std::process::exit(quick_repertoire::app::run_main().await);
}
