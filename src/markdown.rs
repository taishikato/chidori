use scraper::{ElementRef, Html, Selector};

#[derive(Debug, Clone, Copy)]
pub struct MarkdownOptions {
    pub max_chars: Option<usize>,
}

pub fn html_to_markdown(html: &str, options: &MarkdownOptions) -> String {
    let code_block_languages = code_block_languages(html);
    let mut markdown = html2md::parse_html(html);
    markdown = markdown
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .replace("\n\n\n", "\n\n")
        .trim()
        .to_string();
    markdown = normalize_setext_headings(&markdown);
    markdown = annotate_code_fences(&markdown, &code_block_languages);

    if let Some(max_chars) = options.max_chars {
        markdown = markdown.chars().take(max_chars).collect();
    }
    markdown
}

fn normalize_setext_headings(markdown: &str) -> String {
    let lines = markdown.lines().collect::<Vec<_>>();
    let mut normalized = Vec::with_capacity(lines.len());
    let mut index = 0;
    let mut in_fence = false;

    while index < lines.len() {
        if is_backtick_fence(lines[index]) {
            in_fence = !in_fence;
            normalized.push(lines[index].to_string());
            index += 1;
            continue;
        }

        if in_fence {
            normalized.push(lines[index].to_string());
            index += 1;
            continue;
        }

        if let Some(next) = lines.get(index + 1) {
            let marker = next.trim();
            if !lines[index].trim().is_empty()
                && marker.len() >= 3
                && marker.chars().all(|char| char == '=')
            {
                normalized.push(format!("# {}", lines[index].trim()));
                index += 2;
                continue;
            }
            if !lines[index].trim().is_empty()
                && marker.len() >= 3
                && marker.chars().all(|char| char == '-')
            {
                normalized.push(format!("## {}", lines[index].trim()));
                index += 2;
                continue;
            }
        }

        normalized.push(lines[index].to_string());
        index += 1;
    }

    normalized.join("\n")
}

fn is_backtick_fence(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("```")
}

fn annotate_code_fences(markdown: &str, languages: &[Option<String>]) -> String {
    if languages.is_empty() {
        return markdown.to_string();
    }

    let mut language_index = 0;
    let mut in_fence = false;
    let mut annotated = Vec::new();

    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if trimmed == "```" {
            if in_fence {
                in_fence = false;
                annotated.push(line.to_string());
            } else if let Some(language) = languages.get(language_index).and_then(Option::as_ref) {
                in_fence = true;
                language_index += 1;
                annotated.push(format!("```{language}"));
            } else {
                in_fence = true;
                language_index += 1;
                annotated.push(line.to_string());
            }
            continue;
        }

        if trimmed.starts_with("```") {
            in_fence = !in_fence;
        }
        annotated.push(line.to_string());
    }

    annotated.join("\n")
}

fn code_block_languages(html: &str) -> Vec<Option<String>> {
    let fragment = Html::parse_fragment(html);
    let pre_selector = Selector::parse("pre").unwrap();
    let code_selector = Selector::parse("code").unwrap();

    fragment
        .select(&pre_selector)
        .map(|pre| {
            pre.select(&code_selector)
                .next()
                .and_then(language_from_element)
                .or_else(|| language_from_element(pre))
        })
        .collect()
}

fn language_from_element(element: ElementRef<'_>) -> Option<String> {
    element
        .value()
        .attr("data-language")
        .or_else(|| element.value().attr("data-lang"))
        .or_else(|| element.value().attr("lang"))
        .map(str::to_string)
        .or_else(|| class_language(element))
        .and_then(|language| sanitize_language(&language))
}

fn class_language(element: ElementRef<'_>) -> Option<String> {
    element.value().attr("class").and_then(|classes| {
        classes.split_whitespace().find_map(|class_name| {
            class_name
                .strip_prefix("language-")
                .or_else(|| class_name.strip_prefix("lang-"))
                .map(str::to_string)
        })
    })
}

fn sanitize_language(language: &str) -> Option<String> {
    let language = language.trim();
    if language.is_empty() {
        return None;
    }

    let sanitized = language
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '+' | '#' | '.'))
        .collect::<String>();

    (!sanitized.is_empty()).then_some(sanitized)
}
