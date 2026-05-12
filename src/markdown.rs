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
    markdown = collapse_multiline_link_labels(&markdown);
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

fn collapse_multiline_link_labels(markdown: &str) -> String {
    let mut output = String::with_capacity(markdown.len());
    let mut pending = String::new();
    let mut in_fence = false;

    for line in markdown.split_inclusive('\n') {
        let line_without_newline = line.trim_end_matches('\n');
        if is_backtick_fence(line_without_newline) {
            if !in_fence {
                output.push_str(&collapse_multiline_link_labels_in_segment(&pending));
                pending.clear();
            }
            output.push_str(line);
            in_fence = !in_fence;
        } else if in_fence {
            output.push_str(line);
        } else {
            pending.push_str(line);
        }
    }

    if !pending.is_empty() {
        output.push_str(&collapse_multiline_link_labels_in_segment(&pending));
    }

    output
}

fn collapse_multiline_link_labels_in_segment(markdown: &str) -> String {
    let mut output = String::with_capacity(markdown.len());
    let mut index = 0;

    while let Some(relative_start) = markdown[index..].find('[') {
        let start = index + relative_start;
        output.push_str(&markdown[index..start]);

        if markdown[..start].ends_with('!') {
            output.push('[');
            index = start + '['.len_utf8();
            continue;
        }

        let Some((label_end, url_end)) = inline_link_bounds(markdown, start) else {
            output.push('[');
            index = start + '['.len_utf8();
            continue;
        };

        let label = &markdown[start + '['.len_utf8()..label_end];
        if label.contains('\n') {
            output.push('[');
            output.push_str(&normalize_link_label(label));
            output.push_str(&markdown[label_end..=url_end]);
        } else {
            output.push_str(&markdown[start..=url_end]);
        }
        index = url_end + ')'.len_utf8();
    }

    output.push_str(&markdown[index..]);
    output
}

fn inline_link_bounds(markdown: &str, label_start: usize) -> Option<(usize, usize)> {
    let mut escaped = false;
    let mut nested_brackets = 0usize;

    for (relative_offset, ch) in markdown[label_start + '['.len_utf8()..].char_indices() {
        let offset = label_start + '['.len_utf8() + relative_offset;
        if escaped {
            escaped = false;
            continue;
        }

        match ch {
            '\\' => escaped = true,
            '[' => nested_brackets += 1,
            ']' if nested_brackets == 0 => {
                let url_start = offset + ']'.len_utf8();
                if markdown[url_start..].starts_with('(') {
                    let url_end = markdown_url_end(markdown, url_start + '('.len_utf8())?;
                    return Some((offset, url_end));
                }
            }
            ']' => nested_brackets -= 1,
            _ => {}
        }
    }

    None
}

fn normalize_link_label(label: &str) -> String {
    label
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
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
        specialized.replace_figures();
        specialized.replace_math();
        specialized.replace_callouts();
        specialized.replace_footnotes();
        specialized.replace_simple_tables();
        specialized
    }

    fn restore(self, mut markdown: String) -> String {
        markdown = restore_replacements(markdown, &self.replacements);

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
        let placeholder = format!("CHIDORISPECIALPLACEHOLDER{}END", self.replacements.len());
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

    fn replace_figures(&mut self) {
        let source = std::mem::take(&mut self.html);
        let mut output = String::with_capacity(source.len());
        let mut rest = source.as_str();

        while let Some(index) = find_opening_tag(rest, "figure") {
            output.push_str(&rest[..index]);
            let candidate = &rest[index..];
            let Some(open_end) = opening_tag_end(candidate) else {
                output.push_str(candidate);
                self.html = output;
                return;
            };
            let Some(close_end) = find_matching_close(candidate, "figure", open_end + 1) else {
                output.push_str(candidate);
                self.html = output;
                return;
            };
            let fragment = &candidate[..close_end];
            if let Some(markdown) = figure_markdown(fragment) {
                let placeholder = self.push_replacement(markdown);
                output.push_str(&placeholder);
            } else {
                output.push_str(fragment);
            }
            rest = &candidate[close_end..];
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

    fn replace_simple_tables(&mut self) {
        let source = std::mem::take(&mut self.html);
        let mut output = String::with_capacity(source.len());
        let mut rest = source.as_str();

        while let Some(index) = find_opening_tag(rest, "table") {
            output.push_str(&rest[..index]);
            let candidate = &rest[index..];
            let Some(open_end) = opening_tag_end(candidate) else {
                output.push_str(candidate);
                self.html = output;
                return;
            };
            let Some(close_end) = find_matching_close(candidate, "table", open_end + 1) else {
                output.push_str(candidate);
                self.html = output;
                return;
            };
            let fragment = &candidate[..close_end];

            if let Some(markdown) = layout_table_markdown(fragment, &self.replacements) {
                let placeholder = self.push_replacement(markdown);
                output.push_str(&placeholder);
            } else if let Some(markdown) = simple_table_markdown(fragment, &self.replacements) {
                let placeholder = self.push_replacement(markdown);
                output.push_str(&placeholder);
            } else {
                output.push_str(fragment);
            }

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

        loop {
            let next_section = find_opening_tag(rest, "section").map(|index| (index, "section"));
            let next_ordered_list = find_opening_tag(rest, "ol").map(|index| (index, "ol"));
            let Some((index, tag)) = [next_section, next_ordered_list]
                .into_iter()
                .flatten()
                .min_by_key(|(index, _)| *index)
            else {
                break;
            };

            output.push_str(&rest[..index]);
            let candidate = &rest[index..];
            let Some(open_end) = candidate.find('>') else {
                output.push_str(candidate);
                self.html = output;
                return;
            };
            let opening_tag = &candidate[..=open_end];
            let is_standard_footnotes = attr_value(opening_tag, "id").as_deref()
                == Some("footnotes")
                || attr_value(opening_tag, "class")
                    .as_deref()
                    .is_some_and(|classes| {
                        classes
                            .split_whitespace()
                            .any(|class| class == "wp-block-footnotes")
                    });
            if !is_standard_footnotes {
                output.push_str(&candidate[..open_end + 1]);
                rest = &candidate[open_end + 1..];
                continue;
            }
            let Some(close_end) = find_matching_close(candidate, tag, open_end + 1) else {
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

fn figure_markdown(fragment: &str) -> Option<String> {
    let dom = Html::parse_fragment(fragment);
    let img_selector = Selector::parse("img").unwrap();
    let caption_selector = Selector::parse("figcaption").unwrap();
    let images = dom
        .select(&img_selector)
        .map(image_markdown)
        .collect::<Option<Vec<_>>>()?;
    if images.is_empty() {
        return None;
    }
    let caption = dom
        .select(&caption_selector)
        .next()
        .map(|caption| inline_markdown_from_html(&caption.inner_html()))
        .unwrap_or_default();

    let mut parts = images;
    if caption.is_empty() {
        Some(parts.join("\n\n"))
    } else {
        parts.push(caption);
        Some(parts.join("\n\n"))
    }
}

fn image_markdown(img: ElementRef<'_>) -> Option<String> {
    let alt = img.value().attr("alt").unwrap_or("");
    let src = img
        .value()
        .attr("srcset")
        .and_then(largest_srcset_candidate)
        .or_else(|| img.value().attr("src"))?;
    if is_dangerous_url(src) {
        return None;
    }
    Some(format!(
        "![{}]({})",
        escape_markdown_image_alt(alt),
        escape_markdown_image_url(src)
    ))
}

fn escape_markdown_image_alt(value: &str) -> String {
    value
        .replace('\\', r"\\")
        .replace('[', r"\[")
        .replace(']', r"\]")
}

fn escape_markdown_image_url(value: &str) -> String {
    value
        .replace('\\', r"\\")
        .replace('(', r"\(")
        .replace(')', r"\)")
}

fn inline_markdown_from_html(html: &str) -> String {
    html2md::parse_html(&replace_footnote_refs(html))
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn restore_replacements(mut markdown: String, replacements: &[(String, String)]) -> String {
    if replacements.is_empty() {
        return markdown;
    }

    for _ in 0..=replacements.len() {
        let before = markdown.clone();
        for (placeholder, value) in replacements {
            markdown = markdown.replace(placeholder, value);
        }
        if markdown == before {
            break;
        }
    }

    markdown
}

fn layout_table_markdown(fragment: &str, replacements: &[(String, String)]) -> Option<String> {
    let dom = Html::parse_fragment(fragment);
    let table_selector = Selector::parse("table").unwrap();
    let cell_selector = Selector::parse("td, th").unwrap();
    let caption_selector = Selector::parse("caption").unwrap();
    let code_selector = Selector::parse("pre, code").unwrap();
    let table = dom.select(&table_selector).next()?;
    if table.inner_html().to_ascii_lowercase().contains("<table") {
        return None;
    }
    let cells = table.select(&cell_selector).collect::<Vec<_>>();
    if cells.len() != 1 {
        return None;
    }
    if cells[0].value().name().eq_ignore_ascii_case("th") {
        return None;
    }
    let markdown = if table.select(&code_selector).next().is_some() {
        html2md::parse_html(&cells[0].inner_html())
            .trim()
            .to_string()
    } else {
        html2md::parse_html(&cells[0].inner_html())
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    let caption = table
        .select(&caption_selector)
        .next()
        .map(|caption| inline_markdown_from_html(&caption.inner_html()))
        .unwrap_or_default();
    let markdown = if caption.is_empty() {
        markdown
    } else {
        format!("{caption}\n\n{markdown}")
    };
    let markdown = restore_replacements(markdown, replacements);
    (!markdown.is_empty()).then_some(format!("\n{}\n", markdown))
}

struct MarkdownTableCell {
    text: String,
    is_header: bool,
}

fn simple_table_markdown(fragment: &str, replacements: &[(String, String)]) -> Option<String> {
    let dom = Html::parse_fragment(fragment);
    let table_selector = Selector::parse("table").unwrap();
    let row_selector = Selector::parse("tr").unwrap();
    let cell_selector = Selector::parse("th, td").unwrap();
    let table = dom.select(&table_selector).next()?;

    if table.inner_html().to_ascii_lowercase().contains("<table") {
        return None;
    }

    let rows = table
        .select(&row_selector)
        .map(|row| {
            row.select(&cell_selector)
                .map(|cell| {
                    if cell.value().attr("colspan").is_some()
                        || cell.value().attr("rowspan").is_some()
                    {
                        return None;
                    }

                    Some(MarkdownTableCell {
                        text: markdown_table_cell_text(cell, replacements)?,
                        is_header: cell.value().name().eq_ignore_ascii_case("th"),
                    })
                })
                .collect::<Option<Vec<_>>>()
        })
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .filter(|row| !row.is_empty())
        .collect::<Vec<_>>();

    let (header, body_rows) = rows.split_first()?;
    if body_rows.is_empty() || !header.iter().any(|cell| cell.is_header) {
        return None;
    }

    let width = header.len();
    if body_rows.iter().any(|row| row.len() != width) {
        return None;
    }

    let mut output = Vec::with_capacity(body_rows.len() + 2);
    output.push(markdown_table_row(
        header.iter().map(|cell| cell.text.as_str()),
    ));
    output.push(markdown_table_row(std::iter::repeat_n("---", width)));
    for row in body_rows {
        output.push(markdown_table_row(
            row.iter().map(|cell| cell.text.as_str()),
        ));
    }

    Some(format!("\n{}\n", output.join("\n")))
}

fn markdown_table_cell_text(
    cell: ElementRef<'_>,
    replacements: &[(String, String)],
) -> Option<String> {
    let markdown = html2md::parse_html(&cell.inner_html());
    if markdown.lines().any(|line| {
        let trimmed = line.trim_start();
        is_backtick_fence(trimmed) || line.starts_with("    ")
    }) {
        return None;
    }

    let text = markdown
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    Some(restore_replacements(text, replacements).replace('|', "\\|"))
}

fn markdown_table_row<'a>(cells: impl IntoIterator<Item = &'a str>) -> String {
    format!("| {} |", cells.into_iter().collect::<Vec<_>>().join(" | "))
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
        if let Some(id) =
            footnote_id_from_opening_tag(opening_tag).or_else(|| footnote_id_from_html(inner_html))
        {
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

fn footnote_id_from_html(html: &str) -> Option<String> {
    let mut rest = html;

    while let Some(index) = rest.find('<') {
        let candidate = &rest[index..];
        let open_end = opening_tag_end(candidate)?;
        if let Some(id) = footnote_id_from_opening_tag(&candidate[..=open_end]) {
            return Some(id);
        }
        rest = &candidate[open_end + 1..];
    }
    None
}

fn footnote_id_from_opening_tag(opening_tag: &str) -> Option<String> {
    ["href", "id"].into_iter().find_map(|name| {
        attr_value(opening_tag, name).and_then(|value| footnote_id_from_attr_value(&value))
    })
}

fn footnote_id_from_attr_value(value: &str) -> Option<String> {
    if let Some((_, fragment)) = value.rsplit_once('#') {
        return footnote_id_from_prefixed_value(fragment, &["fn-", "footnote-"]);
    }

    footnote_id_from_prefixed_value(value, &["fnref-", "footnote-ref-"])
}

fn footnote_id_from_prefixed_value(value: &str, prefixes: &[&str]) -> Option<String> {
    prefixes.iter().find_map(|prefix| {
        value
            .strip_prefix(prefix)
            .map(footnote_id_suffix)
            .filter(|id| !id.is_empty())
    })
}

fn footnote_id_suffix(value: &str) -> String {
    value
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .collect()
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

fn find_opening_tag(input: &str, tag: &str) -> Option<usize> {
    let mut offset = 0;

    while let Some(index) = input[offset..].find('<') {
        let start = offset + index;
        if tag_name_matches(&input[start + '<'.len_utf8()..], tag) {
            return Some(start);
        }
        offset = start + '<'.len_utf8();
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
