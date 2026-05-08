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

fn assert_occurs_once(markdown: &str, snippet: &str) {
    assert_eq!(
        markdown.matches(snippet).count(),
        1,
        "expected markdown to contain {snippet:?} exactly once\n\n{markdown}"
    );
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
        Case {
            fixture: "domain--hacker-news-listing.html",
            url: "https://news.ycombinator.com/news",
            expected: &[
                "1. [Launch notes for a useful parser](https://example.com/post-one)",
                "123 points",
                "17 comments",
                "2. [Ask HN: Keeping extracted content stable?](https://news.ycombinator.com/item?id=402)",
                "discuss",
            ],
            rejected: &["past", "More"],
        },
        Case {
            fixture: "domain--reddit-discussion.html",
            url: "https://www.reddit.com/r/rust/comments/abc123/example_post/",
            expected: &[
                "Rust ownership finally clicked for me",
                "u/ferrisbuilder",
                "324 upvotes",
                "I kept trying to memorize borrow checker errors",
                "u/systemsreader",
                "91 points · 2 days ago",
                "This is the moment Rust starts feeling like design feedback",
                "> u/borrowedbits",
                "> 28 points · 2 days ago",
                "> That phrasing helped me too",
                "u/macromender",
                "47 points · 2 days ago",
                "Small parser projects are perfect",
            ],
            rejected: &[
                "Log In",
                "Sign Up",
                "About Community",
                "Sponsored course",
                "Reddit app download",
            ],
        },
        Case {
            fixture: "domain--federated-status-thread.html",
            url: "https://mastodon.social/@alice/112233445566778899",
            expected: &[
                "Alice Example",
                "@alice@mastodon.social",
                "May 8, 2026, 3:04 PM",
                "Shipping a tiny parser improvement today.",
                "[release notes](https://example.com/release-notes)",
                "> Bob Builder",
                "> @bob@example.net",
                "> May 8, 2026, 3:20 PM",
                "> This makes saved social threads much easier to read from the CLI.",
            ],
            rejected: &[
                "Explore",
                "Log in",
                "Sign up",
                "Download the official app",
                "New to Mastodon?",
                "Promoted suggestion",
                "Promoted status card",
                "Promoted Person",
                "Mobile apps",
                "Reply",
                "Boost",
                "Favourite",
            ],
        },
    ];

    for case in cases {
        let markdown = fixture_to_markdown(case.fixture, case.url);
        assert_contains_all(&markdown, case.expected);
        assert_contains_none(&markdown, case.rejected);
    }
}

#[test]
fn reddit_reference_reply_is_nested_once() {
    let markdown = fixture_to_markdown(
        "domain--reddit-discussion.html",
        "https://www.reddit.com/r/rust/comments/abc123/example_post/",
    );

    assert_contains_all(
        &markdown,
        &[
            "> u/borrowedbits",
            "> 28 points · 2 days ago",
            "> That phrasing helped me too",
        ],
    );
    assert_occurs_once(&markdown, "u/borrowedbits");
    assert_occurs_once(&markdown, "That phrasing helped me too");
}

#[test]
fn federated_status_reply_is_nested_once() {
    let markdown = fixture_to_markdown(
        "domain--federated-status-thread.html",
        "https://mastodon.social/@alice/112233445566778899",
    );

    assert_contains_all(
        &markdown,
        &[
            "> Bob Builder",
            "> @bob@example.net",
            "> May 8, 2026, 3:20 PM",
            "> This makes saved social threads much easier to read from the CLI.",
        ],
    );
    assert!(!markdown.contains("> Alice Example"));
    assert!(!markdown.contains("Earlier context should not become the primary status."));
    assert!(!markdown.contains("Promoted status card"));
    assert_occurs_once(&markdown, "Alice Example");
    assert_occurs_once(&markdown, "Bob Builder");
    assert_occurs_once(&markdown, "Shipping a tiny parser improvement today.");
    assert_occurs_once(
        &markdown,
        "This makes saved social threads much easier to read from the CLI.",
    );
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
