use crate::error::ChidoriError;
use clap::Parser;
use std::path::PathBuf;
use url::Url;

#[derive(Debug, Parser)]
#[command(name = "chidori")]
#[command(version)]
#[command(about = "Fast Rust-built web-to-Markdown fetcher for coding agents")]
pub struct Cli {
    #[arg(help = "HTTP or HTTPS URL to fetch")]
    pub url: String,

    #[arg(long, help = "Output metadata and Markdown as JSON")]
    pub json: bool,

    #[arg(short, long, help = "Write output to a file")]
    pub output: Option<PathBuf>,

    #[arg(long, help = "Truncate Markdown to a maximum character count")]
    pub max_chars: Option<usize>,

    #[arg(long, default_value_t = 10_000, help = "Set fetch timeout in milliseconds")]
    pub timeout: u64,

    #[arg(long, help = "Override the User-Agent header")]
    pub user_agent: Option<String>,

    #[arg(short = 'l', long = "lang", help = "Set Accept-Language")]
    pub lang: Option<String>,

    #[arg(long, help = "Remove images from Markdown output")]
    pub no_images: bool,

    #[arg(long, help = "Emit extraction diagnostics and timing information")]
    pub debug: bool,
}

#[derive(Debug, Clone)]
pub struct RunConfig {
    pub url: Url,
    pub json: bool,
    pub output: Option<PathBuf>,
    pub max_chars: Option<usize>,
    pub timeout: u64,
    pub user_agent: Option<String>,
    pub lang: Option<String>,
    pub no_images: bool,
    pub debug: bool,
}

impl TryFrom<Cli> for RunConfig {
    type Error = ChidoriError;

    fn try_from(cli: Cli) -> Result<Self, Self::Error> {
        let url = Url::parse(&cli.url).map_err(|_| ChidoriError::InvalidUrl(cli.url.clone()))?;
        match url.scheme() {
            "http" | "https" => {}
            _ => return Err(ChidoriError::InvalidUrl(cli.url)),
        }
        Ok(Self {
            url,
            json: cli.json,
            output: cli.output,
            max_chars: cli.max_chars,
            timeout: cli.timeout,
            user_agent: cli.user_agent,
            lang: cli.lang,
            no_images: cli.no_images,
            debug: cli.debug,
        })
    }
}

pub async fn run(cli: Cli) -> Result<(), ChidoriError> {
    let _config = RunConfig::try_from(cli)?;
    Err(ChidoriError::Unknown(
        "pipeline not implemented".to_string(),
    ))
}
