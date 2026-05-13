#[derive(Debug, Clone)]
pub(crate) struct Candidate {
    pub(crate) diagnostic_id: usize,
    pub(crate) diagnostic_pass: String,
    pub(crate) score: isize,
    pub(crate) selector_index: usize,
    pub(crate) selector: String,
    pub(crate) word_count: usize,
    pub(crate) content_block_count: usize,
    pub(crate) html: String,
}

impl Candidate {
    pub(crate) fn diagnostic_record(&self) -> Self {
        Self {
            diagnostic_id: self.diagnostic_id,
            diagnostic_pass: self.diagnostic_pass.clone(),
            score: self.score,
            selector_index: self.selector_index,
            selector: self.selector.clone(),
            word_count: self.word_count,
            content_block_count: self.content_block_count,
            html: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SiteExtraction {
    pub(crate) selector: String,
    pub(crate) html: String,
}

impl SiteExtraction {
    pub(crate) fn new(selector: impl Into<String>, html: String) -> Self {
        Self {
            selector: selector.into(),
            html,
        }
    }
}

pub(crate) type SiteExtractor = fn(
    &crate::document::ParsedDocument,
) -> Result<Option<SiteExtraction>, crate::error::ChidoriError>;

#[derive(Debug, Clone)]
pub struct ExtractedContent {
    pub html: String,
    pub selector: Option<String>,
    pub score: Option<isize>,
    pub fallbacks: Vec<String>,
    pub diagnostics: ExtractionDiagnostics,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentCandidateDiagnostic {
    pub id: usize,
    pub pass: String,
    pub selector: String,
    pub score: isize,
    pub word_count: usize,
    pub content_block_count: usize,
    pub decision: CandidateDecision,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CandidateDecision {
    Selected,
    Rejected,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FallbackAttemptDiagnostic {
    pub name: String,
    pub accepted: bool,
    pub previous_word_count: usize,
    pub candidate_word_count: usize,
    pub reason: String,
}

#[derive(Debug, Clone, Default)]
pub struct ExtractionDiagnostics {
    pub candidates: Vec<ContentCandidateDiagnostic>,
    pub fallback_attempts: Vec<FallbackAttemptDiagnostic>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn diagnostic_record_does_not_clone_candidate_html() {
        let source = include_str!("types.rs");
        let diagnostic_record_start = source.find("fn diagnostic_record").unwrap();
        let diagnostic_record_end = source[diagnostic_record_start..]
            .find("#[derive(Debug, Clone)]")
            .map(|offset| diagnostic_record_start + offset)
            .unwrap();
        let diagnostic_record_source = &source[diagnostic_record_start..diagnostic_record_end];

        assert!(
            !diagnostic_record_source.contains("..self.clone()"),
            "{diagnostic_record_source}"
        );
    }
}
