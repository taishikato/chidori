use crate::error::ChidoriError;
use crate::fetcher::{fetch_url, FetchConfig, DEFAULT_USER_AGENT};
use clap::Parser;
use std::path::PathBuf;
use std::time::{Duration, Instant};
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

    #[arg(
        long,
        default_value_t = 10_000,
        help = "Set fetch timeout in milliseconds"
    )]
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
    let started = Instant::now();
    let config = RunConfig::try_from(cli)?;
    let fetch_config = FetchConfig {
        timeout: Duration::from_millis(config.timeout),
        max_bytes: 5 * 1024 * 1024,
        user_agent: config
            .user_agent
            .clone()
            .unwrap_or_else(|| DEFAULT_USER_AGENT.to_string()),
        lang: config.lang.clone(),
    };
    let page = fetch_url(&config.url, &fetch_config).await?;
    if config.debug {
        eprintln!(
            "debug: fetched {} in {} ms",
            page.final_url,
            started.elapsed().as_millis()
        );
    }

    let doc = crate::document::ParsedDocument::parse(page.body, page.final_url.clone());
    let mut metadata = crate::metadata::extract_metadata(&doc);
    metadata.url = config.url.to_string();
    metadata.final_url = page.final_url.to_string();

    let main_html = crate::extractor::extract_main_html(&doc)?;
    let cleaned = crate::cleaner::clean_html(
        &main_html,
        &crate::cleaner::CleanOptions {
            no_images: config.no_images,
        },
    );
    let markdown = crate::markdown::html_to_markdown(
        &cleaned,
        &crate::markdown::MarkdownOptions {
            max_chars: config.max_chars,
        },
    );

    if markdown.trim().is_empty() {
        return Err(ChidoriError::ExtractionFailed);
    }

    metadata.word_count = markdown.split_whitespace().count();
    if config.debug {
        eprintln!(
            "debug: extracted {} words in {} ms",
            metadata.word_count,
            started.elapsed().as_millis()
        );
    }

    let mode = if config.json {
        crate::output::RenderMode::Json
    } else {
        crate::output::RenderMode::Markdown
    };
    let rendered = crate::output::render_output(&metadata, &markdown, mode)?;
    crate::output::write_output(config.output.as_deref(), &rendered)
}
