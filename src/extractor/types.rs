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
    use super::Candidate;

    #[test]
    fn diagnostic_record_does_not_clone_candidate_html() {
        let candidate = Candidate {
            diagnostic_id: 7,
            diagnostic_pass: "primary".to_string(),
            score: 42,
            selector_index: 1,
            selector: "article".to_string(),
            word_count: 120,
            content_block_count: 2,
            html: "<article><p>hello</p></article>".to_string(),
        };

        let diagnostic = candidate.diagnostic_record();

        assert!(diagnostic.html.is_empty());
        assert_eq!(diagnostic.diagnostic_id, candidate.diagnostic_id);
        assert_eq!(diagnostic.diagnostic_pass, candidate.diagnostic_pass);
        assert_eq!(diagnostic.score, candidate.score);
        assert_eq!(diagnostic.selector_index, candidate.selector_index);
        assert_eq!(diagnostic.selector, candidate.selector);
        assert_eq!(diagnostic.word_count, candidate.word_count);
        assert_eq!(
            diagnostic.content_block_count,
            candidate.content_block_count
        );
    }
}
