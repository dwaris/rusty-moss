pub fn normalize_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn normalize_text(input: &str) -> String {
    normalize_whitespace(input).to_lowercase()
}

pub fn starts_with_prefix_ignore_ascii_case(text: &str, prefix: &str) -> bool {
    if prefix.is_empty() {
        return true;
    }

    text.get(..prefix.len())
        .map(|candidate| candidate.eq_ignore_ascii_case(prefix))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_whitespace() {
        assert_eq!(normalize_whitespace("  Ash   Prime   Systems  "), "Ash Prime Systems");
    }

    #[test]
    fn test_normalize_text() {
        assert_eq!(normalize_text("  Ash   Prime   Systems  "), "ash prime systems");
    }

    #[test]
    fn test_starts_with_prefix_ignore_ascii_case() {
        assert!(starts_with_prefix_ignore_ascii_case("Lith A1 Relic", "lith a1"));
        assert!(starts_with_prefix_ignore_ascii_case("Lith A1 Relic", ""));
        assert!(!starts_with_prefix_ignore_ascii_case("Lith A1 Relic", "meso"));
    }
}
