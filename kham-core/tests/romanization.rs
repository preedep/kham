use kham_core::fts::FtsTokenizer;
use kham_core::romanizer::RomanizationMap;
use kham_core::stopwords::StopwordSet;

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

// ── FtsTokenizer integration ──────────────────────────────────────────────────

#[test]
fn fts_romanization_appended_to_synonyms() {
    let fts = FtsTokenizer::builder()
        .romanization(RomanizationMap::builtin())
        .stopwords(StopwordSet::from_text(""))
        .build();
    let tokens = fts.segment_for_fts("กิน");
    let t = tokens.iter().find(|t| t.text == "กิน");
    assert!(t.is_some(), "expected 'กิน' token");
    let t = t.unwrap();
    assert!(
        t.synonyms.contains(&String::from("kin")),
        "expected 'kin' in synonyms, got {:?}",
        t.synonyms
    );
}

#[test]
fn fts_without_romanization_no_rtgs_in_synonyms() {
    let fts = FtsTokenizer::builder()
        .stopwords(StopwordSet::from_text(""))
        .build();
    let tokens = fts.segment_for_fts("กิน");
    let t = tokens.iter().find(|t| t.text == "กิน").unwrap();
    assert!(
        !t.synonyms.contains(&String::from("kin")),
        "romanization should not appear when not configured"
    );
}

#[test]
fn fts_romanization_coexists_with_synonyms() {
    let fts = FtsTokenizer::builder()
        .romanization(RomanizationMap::from_tsv("กิน\tkin\n"))
        .synonyms(kham_core::synonym::SynonymMap::from_tsv("กิน\teat\n"))
        .stopwords(StopwordSet::from_text(""))
        .build();
    let tokens = fts.segment_for_fts("กิน");
    let t = tokens.iter().find(|t| t.text == "กิน").unwrap();
    assert!(t.synonyms.contains(&String::from("eat")), "synonym missing");
    assert!(t.synonyms.contains(&String::from("kin")), "rtgs missing");
}

#[test]
fn fts_oov_token_gets_no_rtgs_synonym() {
    let fts = FtsTokenizer::builder()
        .romanization(RomanizationMap::from_tsv("กิน\tkin\n"))
        .stopwords(StopwordSet::from_text(""))
        .build();
    let tokens = fts.segment_for_fts("ข้าว");
    let t = tokens.iter().find(|t| t.text == "ข้าว");
    if let Some(t) = t {
        // "ข้าว" not in our custom map → no RTGS synonym added
        assert!(
            t.synonyms.is_empty(),
            "OOV word should have no synonyms, got {:?}",
            t.synonyms
        );
    }
}

#[test]
fn fts_named_entity_gets_rtgs_synonym() {
    // "ไทย" is PLACE in the NE gazetteer — TokenKind::Named
    let fts = FtsTokenizer::builder()
        .romanization(RomanizationMap::builtin())
        .stopwords(StopwordSet::from_text(""))
        .build();
    let tokens = fts.segment_for_fts("ไทย");
    let t = tokens.iter().find(|t| t.text == "ไทย");
    if let Some(t) = t {
        assert!(
            t.synonyms.contains(&String::from("thai")),
            "Named token 'ไทย' should have RTGS synonym 'thai', got {:?}",
            t.synonyms
        );
    }
}
