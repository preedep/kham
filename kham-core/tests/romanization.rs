use kham_core::romanizer::RomanizationMap;

#[test]
fn builtin_map_is_non_empty() {
    let map = RomanizationMap::builtin();
    assert!(map.len() > 100, "expected at least 100 built-in entries");
}

#[test]
fn common_words_roundtrip() {
    let map = RomanizationMap::builtin();
    let cases = [
        ("กิน", "kin"),
        ("ข้าว", "khao"),
        ("น้ำ", "nam"),
        ("ปลา", "pla"),
        ("ไฟ", "fai"),
        ("ดี", "di"),
        ("มา", "ma"),
        ("ทำ", "tham"),
    ];
    for (thai, rtgs) in cases {
        assert_eq!(map.romanize(thai), Some(rtgs), "failed for {thai}");
    }
}

#[test]
fn unknown_word_is_none() {
    let map = RomanizationMap::builtin();
    assert_eq!(map.romanize("เปปซี่"), None);
    assert_eq!(map.romanize(""), None);
}

#[test]
fn romanize_or_raw_returns_word_when_missing() {
    let map = RomanizationMap::builtin();
    let oov = "เปปซี่";
    assert_eq!(map.romanize_or_raw(oov), oov);
}

#[test]
fn romanize_tokens_mixed_known_and_unknown() {
    let map = RomanizationMap::builtin();
    let tokens = vec!["กิน", "ข้าว", "เปปซี่"];
    let out = map.romanize_tokens(&tokens);
    assert_eq!(out[0], "kin");
    assert_eq!(out[1], "khao");
    assert_eq!(out[2], "เปปซี่");
}

#[test]
fn romanize_tokens_length_matches_input() {
    let map = RomanizationMap::builtin();
    let tokens: Vec<&str> = vec!["กิน", "ข้าว", "ปลา", "น้ำ", "ไฟ"];
    let out = map.romanize_tokens(&tokens);
    assert_eq!(out.len(), tokens.len());
}

#[test]
fn from_tsv_override_last_wins() {
    let map = RomanizationMap::from_tsv("กิน\tkin\nกิน\tgin\n");
    assert_eq!(map.romanize("กิน"), Some("gin"));
}

#[test]
fn from_tsv_trims_whitespace() {
    let map = RomanizationMap::from_tsv("กิน\t kin \n");
    assert_eq!(map.romanize("กิน"), Some("kin"));
}
