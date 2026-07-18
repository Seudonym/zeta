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

pub fn to_pascal_case(s: &str) -> String {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
            }
        })
        .collect()
}

pub fn construct_system_prompt(preamble: String) -> String {
    let cwd = std::env::current_dir()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    format!("{}\nCurrent directory: {}", preamble, cwd)
}

pub fn to_str_arguments(args: serde_json::value::Value) -> String {
    let arguments = args.as_object().expect("failed to parse tool call args");
    let mut args_vec: Vec<_> = arguments.iter().collect();
    args_vec.sort_by_key(|&(key, _)| key);

    args_vec
        .iter()
        .map(|(key, value)| {
            if let Some(string) = value.as_str() {
                format!("({}: {})", key, string.to_string())
            } else {
                format!("({}: {})", key, value.to_string())
            }
        })
        .collect::<Vec<String>>()
        .join(", ")
}
