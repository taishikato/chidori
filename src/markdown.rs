#[derive(Debug, Clone, Copy)]
pub struct MarkdownOptions {
    pub max_chars: Option<usize>,
}

pub fn html_to_markdown(html: &str, options: &MarkdownOptions) -> String {
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

    if let Some(max_chars) = options.max_chars {
        markdown = markdown.chars().take(max_chars).collect();
    }
    markdown
}

fn normalize_setext_headings(markdown: &str) -> String {
    let lines = markdown.lines().collect::<Vec<_>>();
    let mut normalized = Vec::with_capacity(lines.len());
    let mut index = 0;

    while index < lines.len() {
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
