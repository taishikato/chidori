use crate::error::ChidoriError;
use crate::fetcher::{fetch_url, FetchConfig, BOT_USER_AGENT, DEFAULT_USER_AGENT};
use clap::{Parser, ValueEnum};
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{ChildStderr, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};
use url::Url;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

const RENDERER_STDERR_LIMIT: usize = 64 * 1024;

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

    #[arg(long, value_enum, default_value_t = RenderFallback::Off, help = "Use optional external rendering fallback")]
    pub render: RenderFallback,

    #[arg(long, hide = true, help = "Override document URL after fetching")]
    pub source_url: Option<String>,
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
    pub render: RenderFallback,
    pub source_url: Option<Url>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RenderFallback {
    Off,
    Auto,
}

impl TryFrom<Cli> for RunConfig {
    type Error = ChidoriError;

    fn try_from(cli: Cli) -> Result<Self, Self::Error> {
        let url = Url::parse(&cli.url).map_err(|_| ChidoriError::InvalidUrl(cli.url.clone()))?;
        match url.scheme() {
            "http" | "https" => {}
            _ => return Err(ChidoriError::InvalidUrl(cli.url)),
        }
        let source_url = cli
            .source_url
            .as_deref()
            .map(Url::parse)
            .transpose()
            .map_err(|_| ChidoriError::InvalidUrl(cli.source_url.clone().unwrap_or_default()))?;

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
            render: cli.render,
            source_url,
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
    let mut page = match fetch_url(&config.url, &fetch_config).await {
        Ok(page) => page,
        Err(error) => {
            if config.debug {
                eprintln!("debug: fetch failed: {}", classify_fetch_failure(&error));
            }
            return Err(error);
        }
    };
    if config.debug {
        eprintln!(
            "debug: fetched {} in {} ms",
            page.final_url,
            started.elapsed().as_millis()
        );
    }

    let document_url = config
        .source_url
        .clone()
        .unwrap_or_else(|| page.final_url.clone());
    let mut doc = crate::document::ParsedDocument::parse(page.body, document_url);
    let mut fallback_steps = Vec::new();
    let extraction = match extract_markdown_from_doc(&doc, &config) {
        Ok(extraction) => extraction,
        Err(ChidoriError::ExtractionFailed) => {
            if config.debug {
                eprintln!(
                    "debug: extraction failed: {}",
                    classify_extraction_failure(&doc)
                );
            }

            if config.render == RenderFallback::Auto {
                if config.debug {
                    eprintln!("debug: retrying with external renderer");
                }
                match render_with_external_command(
                    &config.url,
                    fetch_config.timeout,
                    fetch_config.max_bytes,
                )
                .and_then(|rendered_html| {
                    let rendered_url = config
                        .source_url
                        .clone()
                        .unwrap_or_else(|| page.final_url.clone());
                    let rendered_doc =
                        crate::document::ParsedDocument::parse(rendered_html, rendered_url);
                    extract_markdown_from_doc(&rendered_doc, &config)
                        .map(|extraction| (rendered_doc, extraction))
                }) {
                    Ok((rendered_doc, extraction)) => {
                        fallback_steps.push("external-renderer".to_string());
                        doc = rendered_doc;
                        extraction
                    }
                    Err(error) if config.user_agent.is_none() => {
                        if config.debug {
                            eprintln!("debug: external renderer failed: {}", error);
                        }
                        fallback_steps.push("bot-user-agent".to_string());
                        retry_with_bot_user_agent(
                            &config,
                            &fetch_config,
                            &mut page.final_url,
                            &mut doc,
                        )
                        .await?
                    }
                    Err(error) => return Err(error),
                }
            } else if config.user_agent.is_none() {
                fallback_steps.push("bot-user-agent".to_string());
                retry_with_bot_user_agent(&config, &fetch_config, &mut page.final_url, &mut doc)
                    .await?
            } else {
                return Err(ChidoriError::ExtractionFailed);
            }
        }
        Err(error) => return Err(error),
    };
    fallback_steps.extend(extraction.fallbacks.iter().cloned());
    let markdown = extraction.markdown;

    let mut metadata = crate::metadata::extract_metadata_with_content_title(
        &doc,
        extraction.content_title.as_deref(),
    );
    metadata.url = config
        .source_url
        .as_ref()
        .unwrap_or(&config.url)
        .to_string();
    metadata.final_url = page.final_url.to_string();

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
    let debug = config.debug.then(|| crate::output::DebugDiagnostics {
        extraction_path: extraction.path.to_string(),
        fallbacks: fallback_steps,
        word_count: metadata.word_count,
        content_selector: extraction.content_selector.clone(),
        content_score: extraction.content_score,
        removals: extraction.removals.clone(),
        timings: crate::output::DebugTimings {
            total_ms: started.elapsed().as_millis(),
        },
    });
    let rendered =
        crate::output::render_output_with_debug(&metadata, &markdown, mode, debug.as_ref())?;
    crate::output::write_output(config.output.as_deref(), &rendered)
}

async fn retry_with_bot_user_agent(
    config: &RunConfig,
    fetch_config: &FetchConfig,
    final_url: &mut Url,
    doc: &mut crate::document::ParsedDocument,
) -> Result<ExtractionResult, ChidoriError> {
    if config.debug {
        eprintln!("debug: retrying with bot user-agent");
    }

    let bot_fetch_config = FetchConfig {
        user_agent: BOT_USER_AGENT.to_string(),
        ..fetch_config.clone()
    };
    match fetch_url(&config.url, &bot_fetch_config).await {
        Ok(bot_page) => {
            let bot_final_url = bot_page.final_url.clone();
            let bot_document_url = config
                .source_url
                .clone()
                .unwrap_or_else(|| bot_final_url.clone());
            let bot_doc = crate::document::ParsedDocument::parse(bot_page.body, bot_document_url);
            match extract_markdown_from_doc(&bot_doc, config) {
                Ok(extraction) => {
                    *final_url = bot_final_url;
                    *doc = bot_doc;
                    Ok(extraction)
                }
                Err(_) => {
                    if config.debug {
                        eprintln!(
                            "debug: extraction failed: {}",
                            classify_extraction_failure(&bot_doc)
                        );
                    }
                    Err(ChidoriError::ExtractionFailed)
                }
            }
        }
        Err(_) => Err(ChidoriError::ExtractionFailed),
    }
}

fn render_with_external_command(
    url: &Url,
    timeout: Duration,
    max_bytes: u64,
) -> Result<String, ChidoriError> {
    let command = std::env::var("CHIDORI_RENDER_COMMAND").map_err(|_| {
        ChidoriError::FetchFailed(
            "CHIDORI_RENDER_COMMAND is required when --render=auto is used".to_string(),
        )
    })?;
    let (program, args) = render_command_parts(&command)?;
    let mut command = Command::new(&program);
    command
        .args(args)
        .arg(url.as_str())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    unsafe {
        command.pre_exec(|| {
            if libc::setpgid(0, 0) == -1 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    let mut child = command
        .spawn()
        .map_err(|error| ChidoriError::FetchFailed(error.to_string()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ChidoriError::FetchFailed("renderer stdout unavailable".to_string()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ChidoriError::FetchFailed("renderer stderr unavailable".to_string()))?;
    let stdout_reader = read_renderer_stdout_in_background(stdout, max_bytes);
    let stderr_reader = read_renderer_stderr_in_background(stderr, RENDERER_STDERR_LIMIT);
    let started = Instant::now();
    let mut status = None;
    let mut output = None;

    loop {
        if status.is_none() {
            status = child
                .try_wait()
                .map_err(|error| ChidoriError::FetchFailed(error.to_string()))?;
        }

        if output.is_none() {
            match stdout_reader.try_recv() {
                Ok(Ok(bytes)) => output = Some(bytes),
                Ok(Err(error)) => {
                    terminate_renderer(&mut child);
                    return Err(error);
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    terminate_renderer(&mut child);
                    return Err(ChidoriError::FetchFailed(
                        "renderer stdout reader stopped".to_string(),
                    ));
                }
            }
        }

        if status.as_ref().is_some_and(|status| !status.success()) {
            let status = status.expect("renderer status checked before loop exits");
            terminate_renderer(&mut child);
            return Err(renderer_exit_error(status, &stderr_reader));
        }

        if status.is_some() && output.is_some() {
            break;
        }

        if started.elapsed() >= timeout {
            terminate_renderer(&mut child);
            return Err(ChidoriError::Timeout(timeout.as_millis() as u64));
        }

        let remaining = timeout.saturating_sub(started.elapsed());
        thread::sleep(remaining.min(Duration::from_millis(10)));
    }

    let status = status.expect("renderer status checked before loop exits");
    debug_assert!(status.success());
    let output = output.expect("renderer output checked before loop exits");

    String::from_utf8(output)
        .map_err(|error| ChidoriError::FetchFailed(format!("renderer returned non-UTF-8: {error}")))
}

fn read_renderer_stdout_in_background(
    mut stdout: ChildStdout,
    max_bytes: u64,
) -> mpsc::Receiver<Result<Vec<u8>, ChidoriError>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut output = Vec::new();
        let mut buffer = [0_u8; 8192];
        let result = loop {
            match stdout.read(&mut buffer) {
                Ok(0) => break Ok(output),
                Ok(bytes_read) => {
                    let actual = output.len() as u64 + bytes_read as u64;
                    if actual > max_bytes {
                        break Err(ChidoriError::TooLarge(actual, max_bytes));
                    }
                    output.extend_from_slice(&buffer[..bytes_read]);
                }
                Err(error) => break Err(ChidoriError::FetchFailed(error.to_string())),
            }
        };
        let _ = sender.send(result);
    });
    receiver
}

fn read_renderer_stderr_in_background(
    mut stderr: ChildStderr,
    max_bytes: usize,
) -> mpsc::Receiver<Vec<u8>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut output = Vec::new();
        let mut buffer = [0_u8; 8192];
        loop {
            match stderr.read(&mut buffer) {
                Ok(0) => break,
                Ok(bytes_read) => {
                    let remaining = max_bytes.saturating_sub(output.len());
                    output.extend_from_slice(&buffer[..bytes_read.min(remaining)]);
                }
                Err(_) => break,
            }
        }
        let _ = sender.send(output);
    });
    receiver
}

fn renderer_exit_error(
    status: ExitStatus,
    stderr_reader: &mpsc::Receiver<Vec<u8>>,
) -> ChidoriError {
    let stderr = stderr_reader
        .recv_timeout(Duration::from_millis(100))
        .unwrap_or_default();
    let stderr = String::from_utf8_lossy(&stderr);
    let stderr = stderr.trim();
    if stderr.is_empty() {
        return ChidoriError::FetchFailed(format!("renderer exited with status {status}"));
    }

    ChidoriError::FetchFailed(format!("renderer exited with status {status}: {stderr}"))
}

fn terminate_renderer(child: &mut std::process::Child) {
    #[cfg(unix)]
    unsafe {
        let _ = libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.try_wait();
}

fn render_command_parts(command: &str) -> Result<(String, Vec<String>), ChidoriError> {
    if Path::new(command).exists() {
        return Ok((command.to_string(), Vec::new()));
    }

    let mut command_parts = shlex::split(command)
        .filter(|parts| !parts.is_empty())
        .ok_or_else(|| ChidoriError::FetchFailed("invalid CHIDORI_RENDER_COMMAND".to_string()))?
        .into_iter();
    let program = command_parts
        .next()
        .ok_or_else(|| ChidoriError::FetchFailed("invalid CHIDORI_RENDER_COMMAND".to_string()))?;
    let args = command_parts.collect::<Vec<_>>();

    Ok((program, args))
}

fn classify_extraction_failure(doc: &crate::document::ParsedDocument) -> &'static str {
    let html = doc.html.to_ascii_lowercase();
    let text_word_count = doc
        .dom
        .root_element()
        .text()
        .filter(|text| !text.trim().is_empty())
        .flat_map(str::split_whitespace)
        .count();

    let has_app_mount = [
        "id=\"root\"",
        "id=\"app\"",
        "id=\"__next\"",
        "id=\"svelte\"",
        "class=\"app\"",
        "data-reactroot",
    ]
    .iter()
    .any(|marker| html.contains(marker));

    if text_word_count < 10 && has_app_mount && html.contains("<script") {
        "spa-shell"
    } else if is_too_link_dense(&doc.html) {
        "too-link-dense"
    } else if text_word_count == 0 {
        "empty-body"
    } else {
        "no-main-candidate"
    }
}

fn classify_fetch_failure(error: &ChidoriError) -> &'static str {
    match error {
        ChidoriError::UnsupportedContentType(_) => "unsupported-content-type",
        ChidoriError::FetchFailed(message)
            if message.starts_with("401 ") || message.starts_with("403 ") =>
        {
            "blocked-or-login"
        }
        ChidoriError::Timeout(_) => "timeout",
        ChidoriError::TooLarge(_, _) => "too-large",
        ChidoriError::InvalidUrl(_) => "invalid-url",
        ChidoriError::FetchFailed(_) => "fetch-failed",
        ChidoriError::ExtractionFailed => "extraction-failed",
        ChidoriError::OutputFailed(_) => "output-failed",
        ChidoriError::Unknown(_) => "unknown",
    }
}

fn is_too_link_dense(html: &str) -> bool {
    let dom = scraper::Html::parse_fragment(html);
    let root = dom.root_element();
    let text = root.text().collect::<Vec<_>>().join(" ");
    let text_len = text.split_whitespace().collect::<String>().len();
    if text_len == 0 {
        return false;
    }

    let Ok(link_selector) = scraper::Selector::parse("a") else {
        return false;
    };
    let links = root.select(&link_selector).collect::<Vec<_>>();
    if links.len() < 20 {
        return false;
    }

    let link_text_len = links
        .iter()
        .map(|link| {
            link.text()
                .collect::<Vec<_>>()
                .join(" ")
                .split_whitespace()
                .collect::<String>()
                .len()
        })
        .sum::<usize>();

    (link_text_len as f64 / text_len as f64) > 0.9
}

fn is_readable_link_dense_content(html: &str, selector: Option<&str>) -> bool {
    if selector.is_some_and(|selector| selector.contains("markdown-body")) {
        return true;
    }

    let dom = scraper::Html::parse_fragment(html);
    let root = dom.root_element();
    let Ok(heading_selector) = scraper::Selector::parse("h1, h2, h3") else {
        return false;
    };
    let heading_text = root
        .select(&heading_selector)
        .map(|heading| heading.text().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join(" ");
    if heading_text.split_whitespace().count() < 2 {
        return false;
    }

    let Ok(item_selector) = scraper::Selector::parse("li") else {
        return false;
    };
    root.select(&item_selector).count() >= 5
}

#[derive(Debug, Clone)]
struct ExtractionResult {
    markdown: String,
    path: ExtractionPath,
    content_selector: Option<String>,
    content_score: Option<isize>,
    removals: Vec<crate::cleaner::RemovalRecord>,
    fallbacks: Vec<String>,
    content_title: Option<String>,
}

#[derive(Debug, Clone, Copy)]
enum ExtractionPath {
    RawMarkdown,
    Html,
}

impl fmt::Display for ExtractionPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RawMarkdown => formatter.write_str("raw-markdown"),
            Self::Html => formatter.write_str("html"),
        }
    }
}

fn extract_markdown_from_doc(
    doc: &crate::document::ParsedDocument,
    config: &RunConfig,
) -> Result<ExtractionResult, ChidoriError> {
    let (markdown, path, content_selector, content_score, removals, fallbacks, content_title) =
        if let Some(raw_markdown) = crate::markdown::extract_raw_markdown(&doc.html) {
            let raw_markdown = if config.no_images {
                crate::markdown::remove_markdown_images(&raw_markdown)
            } else {
                raw_markdown
            };
            let markdown = if let Some(max_chars) = config.max_chars {
                raw_markdown.chars().take(max_chars).collect()
            } else {
                raw_markdown
            };
            let content_title = first_markdown_heading(&markdown);
            (
                markdown,
                ExtractionPath::RawMarkdown,
                None,
                None,
                Vec::new(),
                Vec::new(),
                content_title,
            )
        } else {
            let content = crate::extractor::extract_main_content(doc)?;
            let clean_options = crate::cleaner::CleanOptions {
                no_images: config.no_images,
            };
            let cleaned = if content
                .fallbacks
                .iter()
                .any(|fallback| fallback == "hidden-content")
            {
                crate::cleaner::clean_html_preserving_hidden_with_report(
                    &content.html,
                    &clean_options,
                )
            } else {
                crate::cleaner::clean_html_with_report(&content.html, &clean_options)
            };
            let markdown = crate::markdown::html_to_markdown(
                &cleaned.html,
                &crate::markdown::MarkdownOptions {
                    max_chars: config.max_chars,
                },
            );
            let content_title = crate::metadata::title_from_html_fragment(&cleaned.html);
            if content.score.is_some()
                && is_too_link_dense(&cleaned.html)
                && !is_readable_link_dense_content(&cleaned.html, content.selector.as_deref())
            {
                return Err(ChidoriError::ExtractionFailed);
            }
            (
                markdown,
                ExtractionPath::Html,
                content.selector,
                content.score,
                cleaned.removals,
                content.fallbacks,
                content_title,
            )
        };

    if markdown.trim().is_empty() || is_low_information_spa_shell(doc, &markdown) {
        Err(ChidoriError::ExtractionFailed)
    } else {
        Ok(ExtractionResult {
            markdown,
            path,
            content_selector,
            content_score,
            removals,
            fallbacks,
            content_title,
        })
    }
}

fn first_markdown_heading(markdown: &str) -> Option<String> {
    markdown.lines().find_map(|line| {
        line.strip_prefix("# ")
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(ToString::to_string)
    })
}

fn is_low_information_spa_shell(doc: &crate::document::ParsedDocument, markdown: &str) -> bool {
    if markdown.split_whitespace().count() >= 10 {
        return false;
    }

    let html = doc.html.to_ascii_lowercase();
    html.contains("<script")
        && [
            "id=\"root\"",
            "id=\"app\"",
            "id=\"__next\"",
            "id=\"svelte\"",
            "class=\"app\"",
            "data-reactroot",
        ]
        .iter()
        .any(|marker| html.contains(marker))
}
