use cloud139::utils::*;

#[test]
fn test_str_width_ascii() {
    assert_eq!(str_width("hello"), 5);
    assert_eq!(str_width(""), 0);
    assert_eq!(str_width("a"), 1);
    assert_eq!(str_width("1234567890"), 10);
}

#[test]
fn test_str_width_chinese() {
    assert_eq!(str_width("你好"), 4);
    assert_eq!(str_width("中"), 2);
    assert_eq!(str_width("中文"), 4);
}

#[test]
fn test_str_width_mixed() {
    assert_eq!(str_width("你好hello"), 9);
    assert_eq!(str_width("hello你好"), 9);
    assert_eq!(str_width("a中b文c"), 7);
}

#[test]
fn test_str_width_special_chars() {
    assert_eq!(str_width("!@#$%"), 5);
    assert_eq!(str_width("中文!@"), 6);
}

#[test]
fn test_truncate_with_width_exact() {
    assert_eq!(truncate_with_width("hello", 5), "hello");
    assert_eq!(truncate_with_width("你好", 4), "你好");
}

#[test]
fn test_truncate_with_width_truncate() {
    assert_eq!(truncate_with_width("hello", 3), "hel");
    assert_eq!(truncate_with_width("你好世界", 4), "你好");
    assert_eq!(truncate_with_width("hello", 0), "");
}

#[test]
fn test_truncate_with_width_mixed() {
    assert_eq!(truncate_with_width("hello", 4), "hell");
    assert_eq!(truncate_with_width("你好hello", 6), "你好he");
    assert_eq!(truncate_with_width("h你好", 3), "h你");
}

#[test]
fn test_pad_with_width_exact() {
    assert_eq!(pad_with_width("hello", 5), "hello");
    assert_eq!(pad_with_width("你好", 4), "你好");
}

#[test]
fn test_pad_with_width_pad() {
    assert_eq!(pad_with_width("hello", 10), "hello     ");
    assert_eq!(pad_with_width("你好", 6), "你好  ");
    assert_eq!(pad_with_width("", 5), "     ");
}

#[test]
fn test_pad_with_width_truncate_when_too_long() {
    assert_eq!(pad_with_width("hello", 3), "hel");
    assert_eq!(pad_with_width("你好世界", 4), "你好");
}
