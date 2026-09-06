pub fn normalize_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn normalize_text(input: &str) -> String {
    normalize_whitespace(input).to_lowercase()
}

pub fn starts_with_ignore_case(text: &str, prefix: &str) -> bool {
    if prefix.is_empty() {
        return true;
    }
    let mut text_chars = text.chars();
    for p in prefix.chars() {
        match text_chars.next() {
            Some(t) if t.eq_ignore_ascii_case(&p) => {}
            _ => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starts_with_ignore_case() {
        assert!(starts_with_ignore_case("Lith A1 Relic", "lith a1"));
        assert!(starts_with_ignore_case("Lith A1 Relic", ""));
        assert!(!starts_with_ignore_case("Lith A1 Relic", "meso"));
        // Unicode code point safety (non-ASCII characters spanning multiple bytes)
        assert!(starts_with_ignore_case("日本語テスト", "日本語"));
        assert!(!starts_with_ignore_case("日本語", "日本語テスト"));
    }
}
