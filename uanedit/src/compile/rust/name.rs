/// Words `text` splits into: separators end a word, as does a lower-to-upper camel boundary.
fn words(text: &str) -> Vec<String> {
    let characters: Vec<char> = text.chars().collect();
    let mut words = vec![String::new()];
    for (index, &character) in characters.iter().enumerate() {
        if !character.is_ascii_alphanumeric() {
            if !words.last().expect("word").is_empty() {
                words.push(String::new());
            }
            continue;
        }
        let previous = index
            .checked_sub(1)
            .and_then(|index| characters.get(index))
            .copied();
        let next = characters.get(index + 1).copied();
        let boundary = character.is_ascii_uppercase()
            && match previous {
                Some(previous) if previous.is_ascii_lowercase() || previous.is_ascii_digit() => true,
                Some(previous) if previous.is_ascii_uppercase() => next.is_some_and(|next| next.is_ascii_lowercase()),
                _ => false,
            };
        if boundary && !words.last().expect("word").is_empty() {
            words.push(String::new());
        }
        words.last_mut().expect("word").push(character);
    }
    if words.last().expect("word").is_empty() {
        words.pop();
    }
    words
}

fn guarded(name: String) -> String {
    match name.chars().next() {
        None => "_".to_owned(),
        Some(first) if first.is_ascii_digit() => format!("_{name}"),
        _ => name,
    }
}

pub(super) fn constant(text: &str) -> String {
    guarded(words(text).join("_").to_ascii_uppercase())
}

pub(super) fn type_name(text: &str) -> String {
    let name: String = words(text)
        .iter()
        .map(|word| {
            let mut characters = word.chars();
            match characters.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + characters.as_str(),
                None => String::new(),
            }
        })
        .collect();
    guarded(name)
}

pub(super) fn field(text: &str) -> String {
    let name = guarded(words(text).join("_").to_ascii_lowercase());
    match name.as_str() {
        "self" | "Self" | "super" | "crate" | "_" => format!("{name}_"),
        keyword if KEYWORDS.contains(&keyword) => format!("r#{name}"),
        _ => name,
    }
}

const KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "do", "dyn", "else", "enum",
    "extern", "false", "final", "fn", "for", "gen", "if", "impl", "in", "let", "loop", "macro", "match", "mod", "move",
    "mut", "override", "priv", "pub", "ref", "return", "static", "struct", "trait", "true", "try", "type", "typeof",
    "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

pub(super) fn string_literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            control if control.is_control() => out.push_str(&format!("\\u{{{:x}}}", control as u32)),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}
