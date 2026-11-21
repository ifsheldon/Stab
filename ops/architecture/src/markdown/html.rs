//! Local destinations in raw HTML embedded in Markdown.
//!
//! GitHub renders `<a href>` and `<img src>` in Markdown, so their repository
//! targets need the same checks as Markdown links. This is a small tag scanner,
//! not an HTML parser: it reads one tag at a time within one HTML event, which
//! covers the single-line tags documentation uses. Entity references in values
//! are not decoded, so a value using them fails as a missing target instead of
//! passing unchecked.

/// Returns `(byte offset, destination)` for each `href` of an `a` tag and `src`
/// of an `img` tag in `html`.
pub(super) fn link_destinations(html: &str) -> Vec<(usize, String)> {
    let lower = html.to_ascii_lowercase();
    let mut destinations = Vec::new();
    let mut cursor = 0;
    while let Some(open) = lower.get(cursor..).and_then(|rest| rest.find('<')) {
        let name_start = cursor + open + 1;
        let tag_end = lower
            .get(name_start..)
            .and_then(|rest| rest.find('>'))
            .map_or(lower.len(), |end| name_start + end);
        let tag = lower.get(name_start..tag_end).unwrap_or_default();
        let name_length = tag
            .find(|character: char| character.is_ascii_whitespace() || character == '/')
            .unwrap_or(tag.len());
        let attribute = match tag.get(..name_length) {
            Some("a") => Some("href"),
            Some("img") => Some("src"),
            _ => None,
        };
        if let Some(attribute) = attribute
            && let Some(found) =
                attribute_value(html, &lower, name_start + name_length, tag_end, attribute)
        {
            destinations.push(found);
        }
        if tag_end >= lower.len() {
            break;
        }
        cursor = tag_end + 1;
    }
    destinations
}

fn attribute_value(
    html: &str,
    lower: &str,
    mut cursor: usize,
    end: usize,
    wanted: &str,
) -> Option<(usize, String)> {
    let bytes = lower.as_bytes();
    let byte = |index: usize| bytes.get(index).copied().filter(|_| index < end);
    loop {
        while byte(cursor).is_some_and(|byte| byte.is_ascii_whitespace() || byte == b'/') {
            cursor += 1;
        }
        let name_start = cursor;
        while byte(cursor)
            .is_some_and(|byte| !byte.is_ascii_whitespace() && byte != b'=' && byte != b'/')
        {
            cursor += 1;
        }
        if cursor == name_start {
            return None;
        }
        let name = lower.get(name_start..cursor)?;
        while byte(cursor).is_some_and(|byte| byte.is_ascii_whitespace()) {
            cursor += 1;
        }
        if byte(cursor) != Some(b'=') {
            continue;
        }
        cursor += 1;
        while byte(cursor).is_some_and(|byte| byte.is_ascii_whitespace()) {
            cursor += 1;
        }
        let (value_start, value_end) = match byte(cursor) {
            Some(quote @ (b'"' | b'\'')) => {
                let start = cursor + 1;
                let length = lower
                    .get(start..end)?
                    .bytes()
                    .position(|byte| byte == quote)?;
                cursor = start + length + 1;
                (start, start + length)
            }
            Some(_) => {
                let start = cursor;
                while byte(cursor).is_some_and(|byte| !byte.is_ascii_whitespace()) {
                    cursor += 1;
                }
                (start, cursor)
            }
            None => return None,
        };
        if name == wanted {
            return Some((value_start, html.get(value_start..value_end)?.to_owned()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::link_destinations;

    #[test]
    fn extracts_anchor_and_image_targets_with_offsets() {
        let html = r#"<p><A class="x" HREF="docs/guide.md#part">x</A> <img alt='logo' src=assets/logo.svg/><a name="anchor"></a><area href="skip.md"></p>"#;
        let destinations = link_destinations(html);
        assert_eq!(
            destinations
                .iter()
                .map(|(_, destination)| destination.as_str())
                .collect::<Vec<_>>(),
            ["docs/guide.md#part", "assets/logo.svg/"]
        );
        let (offset, _) = destinations.first().expect("anchor destination");
        assert!(
            html.get(*offset..)
                .is_some_and(|rest| rest.starts_with("docs/guide.md"))
        );
    }
}
