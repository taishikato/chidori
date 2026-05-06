use crate::error::ChidoriError;
use clap::Parser;
use std::path::PathBuf;
use url::Url;

#[derive(Debug, Parser)]
#[command(name = "chidori")]
#[command(version)]
#[command(about = "Fast Rust-built web-to-Markdown fetcher for coding agents")]
pub struct Cli {
    pub url: String,

    #[arg(long)]
    pub json: bool,

    #[arg(short, long)]
    pub output: Option<PathBuf>,

    #[arg(long)]
    pub max_chars: Option<usize>,

    #[arg(long, default_value_t = 10_000)]
    pub timeout: u64,

    #[arg(long)]
    pub user_agent: Option<String>,

    #[arg(short = 'l', long = "lang")]
    pub lang: Option<String>,

    #[arg(long)]
    pub no_images: bool,

    #[arg(long)]
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
