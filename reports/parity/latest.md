# Extraction Parity Report

Generated: 2026-05-11T08:33:30.766Z

Curated cases: 24
Parity or better: 9
Chidori better: 15
Chidori worse: 0
Human review: 0
Tool errors: 0

## Case Results

| Case | Category | Status | Similarity | Words |
| --- | --- | --- | ---: | ---: |
| daringfireball-iphone-16e | main-content-cleanup | parity-or-better | 0.961 | 2127/2075 |
| obsidian-sync-encryption | main-content-cleanup-code | parity-or-better | 0.892 | 811/804 |
| github-pull-request | specialized-content-extraction | chidori-better | 0.407 | 148/52 |
| schema-backed-span-blocks | schema-backed-main-content | parity-or-better | 1 | 98/99 |
| rehype-pretty-copy-code | markdown-code-cleanup | parity-or-better | 0.988 | 137/133 |
| javascript-link-unwrapping | markdown-link-cleanup | chidori-better | 0.941 | 59/55 |
| simple-table | markdown-table-formatting | chidori-better | 0.667 | 23/20 |
| wordpress-footnotes | markdown-footnote-formatting | chidori-better | 1 | 11/11 |
| hidden-visibility-cleanup | hidden-noise-cleanup | parity-or-better | 1 | 62/62 |
| table-of-contents-cleanup | table-of-contents-cleanup | parity-or-better | 1 | 373/355 |
| leading-breadcrumb-cleanup | breadcrumb-cleanup | parity-or-better | 1 | 229/229 |
| trailing-related-links-cleanup | related-links-cleanup | parity-or-better | 1 | 63/62 |
| metadata-dom-author-date | metadata-extraction | chidori-better | 0.913 | 28/25 |
| leetcode-problem | domain-specific-content-extraction | chidori-better | 1 | 38/38 |
| lwn-article | domain-specific-content-extraction | chidori-better | 0.913 | 31/32 |
| ai-conversation | domain-specific-content-extraction | chidori-better | 0.833 | 64/30 |
| hacker-news-listing | domain-specific-content-extraction | chidori-better | 0.683 | 38/47 |
| bluesky-thread | domain-specific-content-extraction | chidori-better | 0.684 | 31/13 |
| threads-post | domain-specific-content-extraction | chidori-better | 0.467 | 21/7 |
| linkedin-post | domain-specific-content-extraction | parity-or-better | 1 | 14/14 |
| reddit-discussion | domain-specific-content-extraction | chidori-better | 0.528 | 128/48 |
| federated-status-thread | domain-specific-content-extraction | chidori-better | 0.552 | 62/73 |
| microblog-status-thread | domain-specific-content-extraction | chidori-better | 0.755 | 79/43 |
| video-watch-page | domain-specific-content-extraction | chidori-better | 0.613 | 51/87 |

## Capability Matrix

| Capability | Chidori status | Evidence |
| --- | --- | --- |
| article entry-point selection, heading preservation, ad/sidebar cleanup | implemented | daringfireball-iphone-16e: parity-or-better |
| article selection, code preservation, navigation cleanup | implemented | obsidian-sync-encryption: parity-or-better |
| discussion thread content preservation without repository chrome | implemented | github-pull-request: chidori-better |
| schema article body fallback with block children preserved | implemented | schema-backed-span-blocks: parity-or-better |
| copy-button cleanup and fenced code language preservation | implemented | rehype-pretty-copy-code: parity-or-better |
| unwrap javascript pseudo-links while preserving inline formatting and real links | implemented | javascript-link-unwrapping: chidori-better |
| simple table Markdown conversion | implemented | simple-table: chidori-better |
| WordPress footnote block Markdown conversion | implemented | wordpress-footnotes: chidori-better |
| remove visibility-hidden content and embedded fallback noise | implemented | hidden-visibility-cleanup: parity-or-better |
| remove fragment-only table-of-contents lists while preserving article sections and code blocks | implemented | table-of-contents-cleanup: parity-or-better |
| remove non-semantic breadcrumb navigation injected into content | implemented | leading-breadcrumb-cleanup: parity-or-better |
| remove short link-dense related-post sections at article end | implemented | trailing-related-links-cleanup: parity-or-better |
| DOM-backed author and published date metadata extraction | implemented | metadata-dom-author-date: chidori-better |
| LeetCode problem statement extraction | implemented | leetcode-problem: chidori-better |
| LWN article extraction | implemented | lwn-article: chidori-better |
| AI conversation transcript extraction | implemented | ai-conversation: chidori-better |
| Hacker News listing extraction | implemented | hacker-news-listing: chidori-better |
| Bluesky thread extraction | implemented | bluesky-thread: chidori-better |
| Threads post extraction | implemented | threads-post: chidori-better |
| LinkedIn post extraction | implemented | linkedin-post: parity-or-better |
| Reddit post and comment extraction | implemented | reddit-discussion: chidori-better |
| federated status/thread extraction | implemented | federated-status-thread: chidori-better |
| X/Twitter-style status/thread extraction | implemented | microblog-status-thread: chidori-better |
| YouTube watch-page extraction | implemented | video-watch-page: chidori-better |

## Open Items

No unexplained chidori-worse-than-reference cases remain in the curated corpus.
