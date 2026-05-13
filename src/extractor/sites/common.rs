use html_escape::encode_text;

pub(super) fn push_meta_paragraph(output: &mut String, parts: &[String]) {
    if parts.is_empty() {
        return;
    }

    output.push_str("<p><small>");
    output.push_str(&encode_text(&parts.join(" · ")));
    output.push_str("</small></p>");
}
