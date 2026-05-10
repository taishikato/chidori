use scraper::{ElementRef, Html, Selector};

#[derive(Debug, Clone, Copy)]
pub struct MarkdownOptions {
    pub max_chars: Option<usize>,
}

pub fn html_to_markdown(html: &str, options: &MarkdownOptions) -> String {
    let specialized = SpecializedHtml::from(html);
    let code_block_languages = code_block_languages(&specialized.html);
    let mut markdown = html2md::parse_html(&specialized.html);
    markdown = markdown
        .lines()
        .map(trim_markdown_line_end)
        .collect::<Vec<_>>()
        .join("\n")
        .replace("\n\n\n", "\n\n")
        .trim()
        .to_string();
    markdown = unwrap_soft_wrapped_paragraphs(&markdown);
    markdown = normalize_setext_headings(&markdown);
    markdown = annotate_code_fences(&markdown, &code_block_languages);
    markdown = specialized.restore(markdown);

    if let Some(max_chars) = options.max_chars {
        markdown = markdown.chars().take(max_chars).collect();
    }
    markdown
}

pub fn extract_raw_markdown(html: &str) -> Option<String> {
    let body = body_inner_html(html)?;
    let without_scripts = remove_raw_tag(
        &remove_raw_tag(&remove_raw_tag(body, "script"), "style"),
        "noscript",
    );
    if has_visible_markup(&without_scripts) {
        return None;
    }
    let text = strip_tags(&without_scripts);
    let markdown = html_escape::decode_html_entities(&text)
        .trim()
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    if !looks_like_markdown(&markdown) {
        return None;
    }
    Some(markdown.replace("\n\n\n", "\n\n").trim().to_string())
}

fn unwrap_soft_wrapped_paragraphs(markdown: &str) -> String {
    let lines = markdown.lines().collect::<Vec<_>>();
    let mut output = Vec::with_capacity(lines.len());
    let mut index = 0;
    let mut in_fence = false;

    while index < lines.len() {
        let line = lines[index];
        if is_backtick_fence(line) {
            in_fence = !in_fence;
            output.push(line.to_string());
            index += 1;
            continue;
        }

        if !in_fence && is_list_item_line(line.trim_start()) {
            let mut item = line.trim_end().to_string();
            index += 1;

            while index < lines.len() && is_paragraph_line(lines[index]) {
                item.push(' ');
                item.push_str(lines[index].trim());
                index += 1;
            }

            output.push(item);
            continue;
        }

        if in_fence || !is_paragraph_line(line) {
            output.push(line.to_string());
            index += 1;
            continue;
        }

        let mut paragraph = line.trim().to_string();
        index += 1;

        while index < lines.len() && is_paragraph_line(lines[index]) {
            paragraph.push(' ');
            paragraph.push_str(lines[index].trim());
            index += 1;
        }

        output.push(paragraph);
    }

    output.join("\n")
}

fn trim_markdown_line_end(line: &str) -> &str {
    if line.ends_with("  ") && !line.trim().is_empty() {
        line
    } else {
        line.trim_end()
    }
}

fn is_paragraph_line(line: &str) -> bool {
    let content = line.trim_start();
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    if content.ends_with("  ") || trimmed.ends_with('\\') {
        return false;
    }
    if is_backtick_fence(trimmed) {
        return false;
    }
    if trimmed.starts_with('|') || trimmed.ends_with('|') {
        return false;
    }
    if trimmed.starts_with('#')
        || trimmed.starts_with('>')
        || trimmed.starts_with("* ")
        || trimmed.starts_with("- ")
        || trimmed.starts_with("+ ")
        || trimmed.starts_with("---")
        || is_setext_heading_marker(trimmed)
    {
        return false;
    }
    if is_ordered_list_line(trimmed) {
        return false;
    }
    true
}

fn is_setext_heading_marker(line: &str) -> bool {
    line.len() >= 3 && (line.chars().all(|ch| ch == '=') || line.chars().all(|ch| ch == '-'))
}

fn is_ordered_list_line(line: &str) -> bool {
    let Some((marker, rest)) = line.split_once(". ") else {
        return false;
    };
    !rest.is_empty() && marker.chars().all(|ch| ch.is_ascii_digit())
}

fn is_list_item_line(line: &str) -> bool {
    line.starts_with("* ")
        || line.starts_with("- ")
        || line.starts_with("+ ")
        || is_ordered_list_line(line)
}

pub fn remove_markdown_images(markdown: &str) -> String {
    let mut output = String::with_capacity(markdown.len());
    let mut index = 0;

    while let Some(relative_start) = markdown[index..].find("![") {
        let start = index + relative_start;
        output.push_str(&markdown[index..start]);
        let Some(label_end) = markdown[start + 2..].find("](").map(|end| start + 2 + end) else {
            output.push_str(&markdown[start..]);
            return output;
        };
        let url_start = label_end + 2;
        let Some(url_end) = markdown_url_end(markdown, url_start) else {
            output.push_str(&markdown[start..]);
            return output;
        };
        index = url_end + 1;
    }

    output.push_str(&markdown[index..]);
    output
}

fn markdown_url_end(markdown: &str, url_start: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, ch) in markdown[url_start..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' if depth == 0 => return Some(url_start + offset),
            ')' => depth -= 1,
            _ => {}
        }
    }
    None
}

fn has_visible_markup(html: &str) -> bool {
    let mut rest = html;
    while let Some(index) = rest.find('<') {
        let candidate = &rest[index..];
        let Some(end) = candidate.find('>') else {
            return false;
        };
        let tag = &candidate[..=end];
        if !tag.starts_with("<!--") && !tag.starts_with("<!") && !tag.starts_with("<?") {
            return true;
        }
        rest = &candidate[end + 1..];
    }
    false
}

fn body_inner_html(html: &str) -> Option<&str> {
    let lower = html.to_ascii_lowercase();
    let body_start = lower.find("<body")?;
    let body_open_end = lower[body_start..].find('>')? + body_start + 1;
    let body_close = lower[body_open_end..].find("</body>")? + body_open_end;
    Some(&html[body_open_end..body_close])
}

fn remove_raw_tag(html: &str, tag: &str) -> String {
    let mut output = String::with_capacity(html.len());
    let mut rest = html;
    let open = format!("<{tag}");
    let close = format!("</{tag}>");

    loop {
        let lower = rest.to_ascii_lowercase();
        let Some(start) = lower.find(&open) else {
            output.push_str(rest);
            return output;
        };
        output.push_str(&rest[..start]);
        let Some(end) = lower[start..].find(&close) else {
            return output;
        };
        rest = &rest[start + end + close.len()..];
    }
}

fn strip_tags(html: &str) -> String {
    let mut output = String::with_capacity(html.len());
    let mut chars = html.char_indices().peekable();

    while let Some((index, ch)) = chars.next() {
        if ch != '<' {
            output.push(ch);
            continue;
        }

        let Some((_, next)) = chars.peek().copied() else {
            output.push(ch);
            continue;
        };
        if !is_tag_start_character(next) {
            output.push(ch);
            continue;
        }
        let Some(close_offset) = html[index..].find('>') else {
            return html.to_string();
        };
        let close_index = index + close_offset;
        while chars
            .peek()
            .is_some_and(|(next_index, _)| *next_index <= close_index)
        {
            chars.next();
        }
    }

    output
}

fn is_tag_start_character(ch: char) -> bool {
    ch.is_ascii_alphabetic() || matches!(ch, '/' | '!' | '?')
}

fn looks_like_markdown(content: &str) -> bool {
    let mut signals = 0;
    if content.lines().any(|line| line.starts_with("# ")) {
        signals += 1;
    }
    if content.contains("**") {
        signals += 1;
    }
    if content.contains("](") {
        signals += 1;
    }
    if content
        .lines()
        .any(|line| line.trim_start().starts_with("- "))
    {
        signals += 1;
    }
    if content
        .lines()
        .any(|line| line.trim_start().starts_with("> "))
    {
        signals += 1;
    }
    if content.contains("```") || content.lines().any(|line| line.starts_with("    ")) {
        signals += 1;
    }
    signals >= 2
}

struct SpecializedHtml {
    html: String,
    replacements: Vec<(String, String)>,
    footnotes: Vec<(String, String)>,
}

impl SpecializedHtml {
    fn from(html: &str) -> Self {
        let mut specialized = Self {
            html: html.to_string(),
            replacements: Vec::new(),
            footnotes: Vec::new(),
        };
        specialized.prefer_largest_srcset_images();
        specialized.replace_math();
        specialized.replace_callouts();
        specialized.replace_footnotes();
        specialized
    }

    fn restore(self, mut markdown: String) -> String {
        for (placeholder, value) in self.replacements {
            markdown = markdown.replace(&placeholder, &value);
        }

        if !self.footnotes.is_empty() {
            markdown.push_str("\n\n---\n\n");
            for (id, text) in self.footnotes {
                markdown.push_str(&format!("[^{id}]: {text}\n\n"));
            }
            markdown = markdown.trim().to_string();
        }

        markdown
    }

    fn push_replacement(&mut self, value: String) -> String {
        let placeholder = format!("CHIDORISPECIAL{}", self.replacements.len());
        self.replacements.push((placeholder.clone(), value));
        placeholder
    }

    fn prefer_largest_srcset_images(&mut self) {
        let source = std::mem::take(&mut self.html);
        let mut output = String::with_capacity(source.len());
        let mut rest = source.as_str();

        while let Some(index) = rest.find("<img") {
            output.push_str(&rest[..index]);
            let candidate = &rest[index..];
            let Some(open_end) = opening_tag_end(candidate) else {
                output.push_str(candidate);
                self.html = output;
                return;
            };
            let opening_tag = &candidate[..=open_end];
            if let Some(best_src) = attr_value(opening_tag, "srcset")
                .and_then(|srcset| largest_srcset_candidate(&srcset).map(ToString::to_string))
            {
                output.push_str(&set_attr_value(opening_tag, "src", &best_src));
            } else {
                output.push_str(opening_tag);
            }
            rest = &candidate[open_end + 1..];
        }

        output.push_str(rest);
        self.html = output;
    }

    fn replace_math(&mut self) {
        let source = std::mem::take(&mut self.html);
        let mut output = String::with_capacity(source.len());
        let mut rest = source.as_str();

        while let Some(index) = rest.find("<math") {
            output.push_str(&rest[..index]);
            let candidate = &rest[index..];
            let Some(open_end) = candidate.find('>') else {
                output.push_str(candidate);
                self.html = output;
                return;
            };
            let opening_tag = &candidate[..=open_end];
            let Some(close_start) = candidate[open_end + 1..].find("</math>") else {
                output.push_str(candidate);
                self.html = output;
                return;
            };
            let content_start = open_end + 1;
            let content_end = content_start + close_start;
            let inner_html = &candidate[content_start..content_end];
            let after = content_end + "</math>".len();
            let latex = attr_value(opening_tag, "data-latex")
                .or_else(|| attr_value(opening_tag, "alttext"))
                .or_else(|| annotation_latex(inner_html))
                .unwrap_or_else(|| text_from_html(inner_html));
            let is_block = attr_value(opening_tag, "display")
                .is_some_and(|display| display.trim().eq_ignore_ascii_case("block"));
            let replacement = if is_block {
                format!("\n$$\n{}\n$$\n", latex.trim())
            } else {
                format!("${}$", latex.trim())
            };
            let placeholder = self.push_replacement(replacement);
            output.push_str(&placeholder);
            rest = &candidate[after..];
        }

        output.push_str(rest);
        self.html = output;
    }

    fn replace_callouts(&mut self) {
        let source = std::mem::take(&mut self.html);
        let mut output = String::with_capacity(source.len());
        let mut rest = source.as_str();

        while let Some(index) = rest.find("<div") {
            output.push_str(&rest[..index]);
            let candidate = &rest[index..];
            let Some(open_end) = candidate.find('>') else {
                output.push_str(candidate);
                self.html = output;
                return;
            };
            let opening_tag = &candidate[..=open_end];
            if !(has_class_token(opening_tag, "callout")
                && attr_value(opening_tag, "data-callout").is_some())
            {
                output.push_str("<div");
                rest = &candidate["<div".len()..];
                continue;
            }
            let Some(close_end) = find_matching_close(candidate, "div", open_end + 1) else {
                output.push_str(candidate);
                self.html = output;
                return;
            };
            let fragment = &candidate[..close_end];
            let kind =
                attr_value(opening_tag, "data-callout").unwrap_or_else(|| "note".to_string());
            let replacement = callout_markdown(fragment, &kind);
            let placeholder = self.push_replacement(replacement);
            output.push_str(&placeholder);
            rest = &candidate[close_end..];
        }

        output.push_str(rest);
        self.html = output;
    }

    fn replace_footnotes(&mut self) {
        self.html = replace_footnote_refs(&self.html);
        let source = std::mem::take(&mut self.html);
        let mut output = String::with_capacity(source.len());
        let mut rest = source.as_str();

        while let Some(index) = rest.find("<section") {
            output.push_str(&rest[..index]);
            let candidate = &rest[index..];
            let Some(open_end) = candidate.find('>') else {
                output.push_str(candidate);
                self.html = output;
                return;
            };
            let opening_tag = &candidate[..=open_end];
            if attr_value(opening_tag, "id").as_deref() != Some("footnotes") {
                output.push_str("<section");
                rest = &candidate["<section".len()..];
                continue;
            }
            let Some(close_end) = find_matching_close(candidate, "section", open_end + 1) else {
                output.push_str(candidate);
                self.html = output;
                return;
            };
            self.footnotes
                .extend(footnotes_from_section(&candidate[..close_end]));
            rest = &candidate[close_end..];
        }

        output.push_str(rest);
        self.html = output;
    }
}

fn callout_markdown(fragment: &str, kind: &str) -> String {
    let dom = Html::parse_fragment(fragment);
    let title_selector = Selector::parse(".callout-title-inner, .callout-title").unwrap();
    let content_selector = Selector::parse(".callout-content").unwrap();
    let title = dom
        .select(&title_selector)
        .next()
        .map(|element| element.text().collect::<Vec<_>>().join(" "))
        .map(|text| text.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_default();
    let content_html = dom
        .select(&content_selector)
        .next()
        .map(|element| element.inner_html())
        .unwrap_or_else(|| fragment.to_string());
    let content_markdown = html2md::parse_html(&content_html);
    let content = content_markdown
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>();
    let mut lines = vec![format!("> [!{}] {}", kind.trim(), title.trim())
        .trim_end()
        .to_string()];
    for line in content {
        lines.push(if line.is_empty() {
            ">".to_string()
        } else {
            format!("> {line}")
        });
    }
    lines.join("\n")
}

fn replace_footnote_refs(html: &str) -> String {
    let mut output = String::with_capacity(html.len());
    let mut rest = html;

    while let Some(index) = rest.find("<sup") {
        output.push_str(&rest[..index]);
        let candidate = &rest[index..];
        let Some(open_end) = candidate.find('>') else {
            output.push_str(candidate);
            return output;
        };
        let opening_tag = &candidate[..=open_end];
        let Some(close_start) = candidate[open_end + 1..].find("</sup>") else {
            output.push_str(candidate);
            return output;
        };
        let content_start = open_end + 1;
        let content_end = content_start + close_start;
        let inner_html = &candidate[content_start..content_end];
        let after = content_end + "</sup>".len();
        if let Some(id) = footnote_id(opening_tag).or_else(|| footnote_id(inner_html)) {
            output.push_str(&format!("[^{id}]"));
        } else {
            output.push_str(&candidate[..after]);
        }
        rest = &candidate[after..];
    }

    output.push_str(rest);
    output
}

fn footnotes_from_section(fragment: &str) -> Vec<(String, String)> {
    let dom = Html::parse_fragment(fragment);
    let item_selector = Selector::parse("li[id]").unwrap();
    dom.select(&item_selector)
        .filter_map(|item| {
            let id = item.value().attr("id").and_then(|id| {
                id.strip_prefix("fn-")
                    .or_else(|| id.strip_prefix("footnote-"))
                    .map(str::to_string)
            })?;
            let mut text = item.text().collect::<Vec<_>>().join(" ");
            text = text.replace('↩', "");
            text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            (!text.is_empty()).then_some((id, text))
        })
        .collect()
}

fn footnote_id(value: &str) -> Option<String> {
    for needle in ["#fn-", "#footnote-", "fnref-", "footnote-ref-"] {
        if let Some(start) = value.find(needle) {
            let id_start = start + needle.len();
            let id = value[id_start..]
                .chars()
                .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
                .collect::<String>();
            if !id.is_empty() {
                return Some(id);
            }
        }
    }
    None
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

fn attr_value(opening_tag: &str, name: &str) -> Option<String> {
    opening_attribute_values(opening_tag, name)
        .next()
        .map(|value| html_escape::decode_html_entities(value).to_string())
}

fn largest_srcset_candidate(srcset: &str) -> Option<&str> {
    srcset
        .split(',')
        .filter_map(|candidate| {
            let mut parts = candidate.split_whitespace();
            let url = parts.next()?;
            if is_dangerous_url(url) {
                return None;
            }
            let descriptor_score = parts.next().and_then(srcset_descriptor_score).unwrap_or(0);
            Some((descriptor_score, url))
        })
        .max_by_key(|(descriptor_score, _url)| *descriptor_score)
        .map(|(_descriptor_score, url)| url)
}

fn is_dangerous_url(value: &str) -> bool {
    let normalized = value
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace() && !ch.is_control())
        .collect::<String>()
        .to_ascii_lowercase();
    normalized.starts_with("javascript:") || normalized.starts_with("data:text/html")
}

fn srcset_descriptor_score(descriptor: &str) -> Option<usize> {
    descriptor
        .strip_suffix('w')
        .and_then(|width| width.parse::<usize>().ok())
        .or_else(|| {
            descriptor.strip_suffix('x').and_then(|density| {
                let density = density.parse::<f64>().ok()?;
                density.is_finite().then_some((density * 1000.0) as usize)
            })
        })
}

fn set_attr_value(opening_tag: &str, name: &str, value: &str) -> String {
    if let Some((value_start, value_end)) = attr_value_range(opening_tag, name) {
        let mut output = String::with_capacity(opening_tag.len() + value.len());
        output.push_str(&opening_tag[..value_start]);
        output.push_str(&html_escape::encode_double_quoted_attribute(value));
        output.push_str(&opening_tag[value_end..]);
        return output;
    }

    insert_attr_value(opening_tag, name, value)
}

fn insert_attr_value(opening_tag: &str, name: &str, value: &str) -> String {
    let close_start = opening_tag.rfind('>').unwrap_or(opening_tag.len());
    let before_close = &opening_tag[..close_start];
    let trimmed_end = before_close.trim_end().len();
    let insert_at = if before_close[..trimmed_end].ends_with('/') {
        trimmed_end.saturating_sub('/'.len_utf8())
    } else {
        close_start
    };

    let escaped = html_escape::encode_double_quoted_attribute(value);
    let mut output = String::with_capacity(opening_tag.len() + name.len() + escaped.len() + 4);
    output.push_str(&opening_tag[..insert_at]);
    output.push(' ');
    output.push_str(name);
    output.push_str("=\"");
    output.push_str(&escaped);
    output.push('"');
    output.push_str(&opening_tag[insert_at..]);
    output
}

fn attr_value_range(opening_tag: &str, expected: &str) -> Option<(usize, usize)> {
    let tag_offset = usize::from(opening_tag.starts_with('<'));
    let input = &opening_tag[tag_offset..];
    let input = input.trim_start();
    let input_offset = tag_offset + opening_tag[tag_offset..].len() - input.len();
    let name_end = input
        .find(|ch: char| ch.is_ascii_whitespace() || ch == '/')
        .unwrap_or(input.len());
    let mut offset = input_offset + name_end;

    while offset < opening_tag.len() {
        let rest = &opening_tag[offset..];
        let trimmed = rest.trim_start();
        offset += rest.len() - trimmed.len();
        if trimmed.is_empty() || trimmed.starts_with('/') || trimmed.starts_with('>') {
            return None;
        }

        let name_end = trimmed
            .find(|ch: char| ch.is_ascii_whitespace() || ch == '=' || ch == '/' || ch == '>')
            .unwrap_or(trimmed.len());
        if name_end == 0 {
            offset += trimmed.chars().next()?.len_utf8();
            continue;
        }

        let attr_name = &trimmed[..name_end];
        offset += name_end;
        let rest = &opening_tag[offset..];
        let trimmed = rest.trim_start();
        offset += rest.len() - trimmed.len();
        if !trimmed.starts_with('=') {
            continue;
        }

        offset += '='.len_utf8();
        let rest = &opening_tag[offset..];
        let trimmed = rest.trim_start();
        offset += rest.len() - trimmed.len();
        if trimmed.is_empty() {
            return None;
        }

        let value_start;
        let value_end;
        if let Some(quote) = trimmed
            .chars()
            .next()
            .filter(|quote| matches!(quote, '"' | '\''))
        {
            value_start = offset + quote.len_utf8();
            let value = &opening_tag[value_start..];
            let end = value.find(quote)?;
            value_end = value_start + end;
            offset = value_end + quote.len_utf8();
        } else {
            value_start = offset;
            let value = &opening_tag[value_start..];
            let end = value
                .find(|ch: char| ch.is_ascii_whitespace() || ch == '/' || ch == '>')
                .unwrap_or(value.len());
            value_end = value_start + end;
            offset = value_end;
        }

        if attr_name.eq_ignore_ascii_case(expected) {
            return Some((value_start, value_end));
        }
    }

    None
}

fn opening_tag_end(input: &str) -> Option<usize> {
    let mut quote: Option<char> = None;

    for (index, character) in input.char_indices() {
        match quote {
            Some(current) if character == current => quote = None,
            Some(_) => {}
            None if character == '"' || character == '\'' => quote = Some(character),
            None if character == '>' => return Some(index),
            None => {}
        }
    }

    None
}

fn opening_attribute_values<'a>(
    opening_tag: &'a str,
    expected: &'a str,
) -> impl Iterator<Item = &'a str> + 'a {
    OpeningAttributes::new(opening_tag).filter_map(move |(name, value)| {
        name.eq_ignore_ascii_case(expected)
            .then_some(value)
            .flatten()
    })
}

struct OpeningAttributes<'a> {
    input: &'a str,
}

impl<'a> OpeningAttributes<'a> {
    fn new(opening_tag: &'a str) -> Self {
        let input = opening_tag
            .strip_prefix('<')
            .and_then(|value| value.strip_suffix('>'))
            .unwrap_or(opening_tag);
        let input = input.trim_start().trim_end_matches('/').trim_end();
        let name_end = input
            .find(|ch: char| ch.is_ascii_whitespace() || ch == '/')
            .unwrap_or(input.len());

        Self {
            input: &input[name_end..],
        }
    }
}

impl<'a> Iterator for OpeningAttributes<'a> {
    type Item = (&'a str, Option<&'a str>);

    fn next(&mut self) -> Option<Self::Item> {
        self.input = self.input.trim_start();
        if self.input.is_empty() || self.input.starts_with('/') {
            return None;
        }

        let name_end = self
            .input
            .find(|ch: char| ch.is_ascii_whitespace() || ch == '=' || ch == '/')
            .unwrap_or(self.input.len());
        if name_end == 0 {
            self.input = &self.input[1..];
            return self.next();
        }

        let name = &self.input[..name_end];
        let mut rest = self.input[name_end..].trim_start();
        if !rest.starts_with('=') {
            self.input = rest;
            return Some((name, None));
        }

        rest = rest[1..].trim_start();
        if rest.is_empty() {
            self.input = rest;
            return Some((name, Some("")));
        }

        if let Some(quote) = rest
            .chars()
            .next()
            .filter(|quote| matches!(quote, '"' | '\''))
        {
            let value = &rest[quote.len_utf8()..];
            if let Some(end) = value.find(quote) {
                self.input = &value[end + quote.len_utf8()..];
                return Some((name, Some(&value[..end])));
            }
            self.input = "";
            return Some((name, Some(value)));
        }

        let value_end = rest
            .find(|ch: char| ch.is_ascii_whitespace() || ch == '/')
            .unwrap_or(rest.len());
        self.input = &rest[value_end..];
        Some((name, Some(&rest[..value_end])))
    }
}

fn annotation_latex(html: &str) -> Option<String> {
    let dom = Html::parse_fragment(html);
    let selector = Selector::parse("annotation").ok()?;
    dom.select(&selector).find_map(|annotation| {
        annotation
            .value()
            .attr("encoding")
            .filter(|encoding| encoding.eq_ignore_ascii_case("application/x-tex"))
            .map(|_| {
                annotation
                    .text()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .filter(|text| !text.is_empty())
    })
}

fn text_from_html(html: &str) -> String {
    Html::parse_fragment(html)
        .root_element()
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn has_class_token(opening_tag: &str, expected: &str) -> bool {
    attr_value(opening_tag, "class").is_some_and(|classes| {
        classes
            .split_ascii_whitespace()
            .any(|token| token == expected)
    })
}

fn find_matching_close(html: &str, tag: &str, search_start: usize) -> Option<usize> {
    let mut depth = 1;
    let mut offset = search_start;

    while let Some(index) = html[offset..].find('<') {
        let start = offset + index;
        let candidate = &html[start..];

        if closing_tag_name_matches(&candidate[1..], tag) {
            if let Some(end) = candidate.find('>') {
                depth -= 1;
                if depth == 0 {
                    return Some(start + end + 1);
                }
                offset = start + end + 1;
                continue;
            }
        } else if tag_name_matches(&candidate[1..], tag) {
            if let Some(end) = candidate.find('>') {
                if !candidate[..=end].ends_with("/>") {
                    depth += 1;
                }
                offset = start + end + 1;
                continue;
            }
        }

        offset = start + 1;
    }

    None
}

fn tag_name_matches(input: &str, tag: &str) -> bool {
    let mut chars = input.chars();
    for expected in tag.chars() {
        match chars.next() {
            Some(actual) if actual.eq_ignore_ascii_case(&expected) => {}
            _ => return false,
        }
    }

    chars
        .next()
        .is_some_and(|ch| ch.is_ascii_whitespace() || ch == '>' || ch == '/')
}

fn closing_tag_name_matches(input: &str, tag: &str) -> bool {
    input
        .strip_prefix('/')
        .is_some_and(|rest| tag_name_matches(rest, tag))
}
