use chidori::cli::Cli;
use clap::Parser;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(error) = chidori::cli::run(cli).await {
        eprintln!("Error: {}", error);
        std::process::exit(error.exit_code());
    }
}
