use chidori::{
    cleaner::{clean_html, CleanOptions},
    document::ParsedDocument,
    extractor::extract_main_html,
    markdown::{html_to_markdown, MarkdownOptions},
};
use url::Url;

fn fixture_to_markdown(fixture: &str, url: &str) -> String {
    let html = std::fs::read_to_string(format!("tests/fixtures/reference/{fixture}")).unwrap();
    let doc = ParsedDocument::parse(html, Url::parse(url).unwrap());
    let main = extract_main_html(&doc).unwrap();
    let cleaned = clean_html(&main, &CleanOptions { no_images: false });

    html_to_markdown(&cleaned, &MarkdownOptions { max_chars: None })
}

fn assert_contains_all(markdown: &str, snippets: &[&str]) {
    for snippet in snippets {
        assert!(
            markdown.contains(snippet),
            "expected markdown to contain {snippet:?}\n\n{markdown}"
        );
    }
}

fn assert_contains_none(markdown: &str, snippets: &[&str]) {
    for snippet in snippets {
        assert!(
            !markdown.contains(snippet),
            "expected markdown not to contain {snippet:?}\n\n{markdown}"
        );
    }
}

#[test]
fn matches_reference_pages_for_representative_urls() {
    struct Case<'a> {
        fixture: &'a str,
        url: &'a str,
        expected: &'a [&'a str],
        rejected: &'a [&'a str],
    }

    let cases = [
        Case {
            fixture: "general--daringfireball.net-2025-02-the_iphone_16e.html",
            url: "https://daringfireball.net/2025/02/the_iphone_16e",
            expected: &[
                "The 16e camera lens is not flush with the back of the phone",
                "## What’s Missing: MagSafe, ProMotion, and Ultra Wideband",
                "iPhone 16e",
                "7.8mm",
            ],
            rejected: &["WorkOS Pipes", "Previous articles.", "SiteSearch"],
        },
        Case {
            fixture: "general--obsidian.md-blog-verify-obsidian-sync-encryption.html",
            url: "https://obsidian.md/blog/verify-obsidian-sync-encryption/",
            expected: &[
                "Obsidian Sync encryption",
                "crypto.scryptSync",
                "The salt of your vault Notes is",
            ],
            rejected: &["Download Obsidian", "Log in"],
        },
        Case {
            fixture: "general--github.com-test-owner-test-repo-pull-42.html",
            url: "https://github.com/test-owner/test-repo/pull/42",
            expected: &[
                "## Summary",
                "This fixes a regression where content was clipped partway through extraction.",
                "Consider removing just the image element instead of the entire anchor",
                "Posted a follow-up commit to address the review comments.",
            ],
            rejected: &["Pull requests · test-owner/test-repo"],
        },
        Case {
            fixture: "issues--span-with-block-children-and-schema.html",
            url: "https://example.org/post-about-systems",
            expected: &[
                "Systems come in many forms.",
                "### Rigid",
                "Elastic systems absorb stress but have limits.",
            ],
            rejected: &["Related posts", "brief summary of another post"],
        },
        Case {
            fixture: "elements--javascript-links.html",
            url: "https://example.com/javascript-links",
            expected: &[
                "This has a simple js link in a sentence.",
                "A **bold js link** should keep its inner HTML.",
                "[Example](https://example.com)",
            ],
            rejected: &["javascript:void", "javascript:alert"],
        },
        Case {
            fixture: "hidden--visibility.html",
            url: "https://example.com/visibility-hidden",
            expected: &[
                "## Foo",
                "Tempor incididunt ut labore et dolore magna aliqua.",
                "Duis aute irure dolor in reprehenderit in voluptate velit esse",
            ],
            rejected: &[
                "Lorem ipsum dolor sit amet, consectetur adipisicing elit",
                "Iframe fallback test",
                "foo.swf",
            ],
        },
        Case {
            fixture: "content-patterns--table-of-contents.html",
            url: "https://www.example.org/install-guide/",
            expected: &[
                "# Installation Guide",
                "The system is installed as the sole operating system",
                "```",
                "sha256sum -c --ignore-missing sha256sums.txt",
            ],
            rejected: &[
                "[1. Start Here](#1-start-here)",
                "[Acquire the image](#acquire-the-image)",
            ],
        },
        Case {
            fixture: "content-patterns--leading-breadcrumb.html",
            url: "https://example.com/newsletter-post",
            expected: &[
                "Not a shadowing day or research interview",
                "## Why this industry?",
                "## Getting the job",
            ],
            rejected: &["[Home](/)", "[Posts](/archive)"],
        },
        Case {
            fixture: "content-patterns--trailing-related-posts.html",
            url: "https://example.com/coffee-cooling",
            expected: &[
                "# How Coffee Cools",
                "Coffee cools following Newton's law of cooling",
                "Most models fit two exponential decay terms",
            ],
            rejected: &["Maybe there's a pattern here?", "The real data wall"],
        },
    ];

    for case in cases {
        let markdown = fixture_to_markdown(case.fixture, case.url);
        assert_contains_all(&markdown, case.expected);
        assert_contains_none(&markdown, case.rejected);
    }
}

#[test]
fn matches_reference_rehype_pretty_copy_code_block_output() {
    let markdown = fixture_to_markdown(
        "codeblocks--rehype-pretty-copy.html",
        "https://example.com/weekly-project-review",
    );

    assert_contains_all(
        &markdown,
        &[
            "The rehype-pretty-copy plugin injects a copy button",
            "```yaml",
            "tags:\n  - Projects/Open",
            "complete-date:            # set when marked done",
        ],
    );
    assert_contains_none(
        &markdown,
        &["Copy code", "navigator.clipboard", "--copy-icon"],
    );
}
