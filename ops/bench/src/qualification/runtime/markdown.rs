pub(super) fn inline_code(value: &str) -> String {
    let value = value.replace(['\r', '\n'], " ").replace('|', "\\|");
    let longest_run = value
        .split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or(0);
    let fence = "`".repeat(longest_run.saturating_add(1));
    format!("{fence} {value} {fence}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_code_blocks_table_and_line_injection() {
        let rendered = inline_code("cpu\n|`identity``");
        assert!(!rendered.contains('\n'));
        assert!(rendered.contains("\\|"));
        assert!(rendered.starts_with("```"));
        assert!(rendered.ends_with("```"));
    }
}
