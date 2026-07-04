pub fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max_chars).collect();
        out.push_str("\n      ... [truncated]");
        out
    }
}

pub fn indent(s: &str, spaces: usize) -> String {
    let pad: String = " ".repeat(spaces);
    s.lines()
        .map(|line| format!("{}{}", pad, line))
        .collect::<Vec<_>>()
        .join("\n")
}
