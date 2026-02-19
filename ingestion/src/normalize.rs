use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

pub fn norm_display(input: &str) -> String {
    collapse_whitespace(input.nfkc().collect::<String>().trim())
}

pub fn key_strict(norm: &str) -> String {
    norm.to_lowercase()
}

pub fn key_folded(key_strict: &str) -> String {
    key_strict
        .nfkd()
        .filter(|&ch| !is_combining_mark(ch))
        .filter(|ch| ch.is_alphanumeric() || *ch == '-')
        .collect()
}

fn collapse_whitespace(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_was_space = false;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !prev_was_space {
                out.push(' ');
            }
            prev_was_space = true;
        } else {
            out.push(ch);
            prev_was_space = false;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_accents_and_punctuation_for_key() {
        let norm = norm_display("  Abdón  ");
        assert_eq!(norm, "Abdón");
        assert_eq!(key_strict(&norm), "abdón");
        assert_eq!(key_folded("abdón"), "abdon");

        let norm = norm_display("ABD'ALLAH");
        assert_eq!(key_folded(&key_strict(&norm)), "abdallah");

        let norm = norm_display("ABD-ALLAH");
        assert_eq!(key_folded(&key_strict(&norm)), "abd-allah");

        let norm = norm_display("Anne-Marie");
        assert_eq!(key_folded(&key_strict(&norm)), "anne-marie");
    }

    #[test]
    fn collapses_internal_whitespace() {
        assert_eq!(norm_display("Juan   Ignacio"), "Juan Ignacio");
        assert_eq!(
            key_folded(&key_strict(&norm_display("Juan Ignacio"))),
            "juanignacio"
        );
    }
}
