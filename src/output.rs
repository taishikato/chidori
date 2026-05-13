use crate::{error::ChidoriError, metadata::Metadata};
use serde::Serialize;
use std::{fs, path::Path};

#[derive(Debug, Clone, Copy)]
pub enum RenderMode {
    Markdown,
    Json,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugDiagnostics {
    pub extraction_path: String,
    pub fallbacks: Vec<String>,
    pub word_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_selector: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_score: Option<isize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_class: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub removals: Vec<crate::cleaner::RemovalRecord>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub standardizations: Vec<crate::standardize::StandardizeRecord>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<crate::extractor::ContentCandidateDiagnostic>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fallback_attempts: Vec<crate::extractor::FallbackAttemptDiagnostic>,
    pub timings: DebugTimings,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugTimings {
    pub total_ms: u128,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonOutput<'a> {
    #[serde(flatten)]
    metadata: &'a Metadata,
    markdown: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    debug: Option<&'a DebugDiagnostics>,
}

pub fn render_output(
    metadata: &Metadata,
    markdown: &str,
    mode: RenderMode,
) -> Result<String, ChidoriError> {
    render_output_with_debug(metadata, markdown, mode, None)
}

pub fn render_output_with_debug(
    metadata: &Metadata,
    markdown: &str,
    mode: RenderMode,
    debug: Option<&DebugDiagnostics>,
) -> Result<String, ChidoriError> {
    match mode {
        RenderMode::Markdown => Ok(markdown.to_string()),
        RenderMode::Json => serde_json::to_string_pretty(&JsonOutput {
            metadata,
            markdown,
            debug,
        })
        .map_err(|error| ChidoriError::OutputFailed(error.to_string())),
    }
}

pub fn write_output(path: Option<&Path>, output: &str) -> Result<(), ChidoriError> {
    if let Some(path) = path {
        fs::write(path, output).map_err(|error| {
            ChidoriError::OutputFailed(format!("failed to write {}: {}", path.display(), error))
        })?;
    } else {
        print!("{}", output);
    }
    Ok(())
}
