use crate::{error::ChidoriError, metadata::Metadata};
use serde::Serialize;
use std::{fs, path::Path};

#[derive(Debug, Clone, Copy)]
pub enum RenderMode {
    Markdown,
    Json,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonOutput<'a> {
    #[serde(flatten)]
    metadata: &'a Metadata,
    markdown: &'a str,
}

pub fn render_output(
    metadata: &Metadata,
    markdown: &str,
    mode: RenderMode,
) -> Result<String, ChidoriError> {
    match mode {
        RenderMode::Markdown => Ok(markdown.to_string()),
        RenderMode::Json => serde_json::to_string_pretty(&JsonOutput { metadata, markdown })
            .map_err(|error| ChidoriError::OutputFailed(error.to_string())),
    }
}

pub fn write_output(path: Option<&Path>, output: &str) -> Result<(), ChidoriError> {
    if let Some(path) = path {
        fs::write(path, output).map_err(|error| ChidoriError::OutputFailed(error.to_string()))?;
    } else {
        println!("{}", output);
    }
    Ok(())
}
