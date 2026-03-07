pub fn str_width(s: &str) -> usize {
    s.chars().map(|c| if c.is_ascii() { 1 } else { 2 }).sum()
}

pub fn truncate_with_width(s: &str, max_width: usize) -> String {
    let mut width = 0;
    let mut result = String::new();
    for c in s.chars() {
        let char_width = if c.is_ascii() { 1 } else { 2 };
        if width + char_width > max_width {
            break;
        }
        result.push(c);
        width += char_width;
    }
    result
}

pub fn pad_with_width(s: &str, width: usize) -> String {
    let current_width = str_width(s);
    if current_width >= width {
        truncate_with_width(s, width)
    } else {
        let padding = width - current_width;
        format!("{}{:width$}", s, "", width = padding)
    }
}
