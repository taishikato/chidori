use crate::{document::ParsedDocument, error::ChidoriError};

use super::types::{SiteExtraction, SiteExtractor};

mod common;
mod discourse;
mod github;
mod known;
mod mastodon;
mod microblog;
mod reddit;
mod social;
mod video;

const SITE_EXTRACTORS: &[SiteExtractor] = &[
    video::extract,
    microblog::extract,
    mastodon::extract,
    github::extract,
    social::extract,
    discourse::extract,
    known::extract,
    reddit::extract,
];

pub(super) fn extract_site_content(
    doc: &ParsedDocument,
) -> Result<Option<SiteExtraction>, ChidoriError> {
    for extractor in SITE_EXTRACTORS {
        if let Some(extraction) = extractor(doc)? {
            return Ok(Some(extraction));
        }
    }

    Ok(None)
}
