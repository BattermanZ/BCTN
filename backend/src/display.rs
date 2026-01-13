pub(crate) fn clean_spelling_display(original: &str) -> String {
    // Only rewrite casing when the input has *no* lowercase letters (typical of some data sources).
    // This avoids breaking legitimate mixed-case spellings like "McKenna" or "DeAndre".
    if original.chars().any(|c| c.is_lowercase()) {
        return original.to_string();
    }

    let mut out = String::with_capacity(original.len());
    let mut capitalize_next = true;

    for ch in original.chars() {
        if ch.is_alphabetic() {
            if capitalize_next {
                out.extend(ch.to_uppercase());
                capitalize_next = false;
            } else {
                out.extend(ch.to_lowercase());
            }
            continue;
        }

        out.push(ch);
        if matches!(ch, ' ' | '-' | '\'') {
            capitalize_next = true;
        }
    }

    out
}

pub(crate) fn dedup_preserve_order(values: Vec<String>) -> Vec<String> {
    use std::collections::HashSet;
    let mut seen = HashSet::<String>::new();
    let mut out = Vec::with_capacity(values.len());
    for v in values {
        if seen.insert(v.clone()) {
            out.push(v);
        }
    }
    out
}
