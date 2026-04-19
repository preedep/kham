use kham_core::fts::FtsTokenizer;
use kham_core::ne::NeTagger;
use kham_core::token::{NamedEntityKind, TokenKind};

#[test]
fn builtin_gazetteer_non_empty() {
    assert!(NeTagger::builtin().len() > 50);
}

#[test]
fn place_country_tagged() {
    let t = NeTagger::builtin();
    assert_eq!(t.tag("ไทย"), Some(NamedEntityKind::Place));
    assert_eq!(t.tag("ญี่ปุ่น"), Some(NamedEntityKind::Place));
    assert_eq!(t.tag("กรุงเทพ"), Some(NamedEntityKind::Place));
}

#[test]
fn org_tagged() {
    let t = NeTagger::builtin();
    assert_eq!(t.tag("ปตท"), Some(NamedEntityKind::Org));
    assert_eq!(t.tag("ธนาคารแห่งประเทศไทย"), Some(NamedEntityKind::Org));
}

#[test]
fn person_tagged() {
    let t = NeTagger::builtin();
    assert_eq!(t.tag("ทักษิณ"), Some(NamedEntityKind::Person));
}

#[test]
fn common_word_not_tagged() {
    let t = NeTagger::builtin();
    assert_eq!(t.tag("กิน"), None);
    assert_eq!(t.tag("บ้าน"), None);
}

#[test]
fn ne_kind_roundtrip() {
    for kind in [
        NamedEntityKind::Person,
        NamedEntityKind::Place,
        NamedEntityKind::Org,
    ] {
        assert_eq!(NamedEntityKind::from_tag(kind.as_tag()), Some(kind));
        assert!(!kind.as_str().is_empty());
    }
}

#[test]
fn fts_token_kind_is_named_for_ne() {
    let fts = FtsTokenizer::new();
    // ไทย segments as a single token and is PLACE in the gazetteer
    let tokens = fts.segment_for_fts("ไทย");
    let t = tokens.iter().find(|t| t.text == "ไทย");
    assert!(t.is_some(), "expected 'ไทย' token");
    let t = t.unwrap();
    assert_eq!(t.kind, TokenKind::Named(NamedEntityKind::Place));
    assert_eq!(t.ne, Some(NamedEntityKind::Place));
}

#[test]
fn fts_token_ne_none_for_common_word() {
    let fts = FtsTokenizer::new();
    let tokens = fts.segment_for_fts("กินข้าว");
    for t in &tokens {
        assert!(t.ne.is_none(), "common word '{}' should have no NE", t.text);
    }
}

#[test]
fn fts_builder_custom_ne_tagger() {
    let tagger = NeTagger::from_tsv("กิน\tPERSON\n"); // intentionally odd, just for test
    let fts = FtsTokenizer::builder()
        .ne_tagger(tagger)
        .stopwords(kham_core::stopwords::StopwordSet::from_text(""))
        .build();
    let tokens = fts.segment_for_fts("กิน");
    let gin = tokens.iter().find(|t| t.text == "กิน");
    if let Some(t) = gin {
        assert_eq!(t.kind, TokenKind::Named(NamedEntityKind::Person));
        assert_eq!(t.ne, Some(NamedEntityKind::Person));
    }
}

#[test]
fn ne_token_has_no_pos() {
    let fts = FtsTokenizer::new();
    let tokens = fts.segment_for_fts("กรุงเทพ");
    let t = tokens.iter().find(|t| t.text == "กรุงเทพ");
    if let Some(t) = t {
        // Named tokens are not Thai, so POS tagger skips them
        assert!(t.pos.is_none(), "NE token should not have a POS tag");
    }
}

// ── Gap 1: tag_tokens passes through all non-Thai kinds ──────────────────────

#[test]
fn tag_tokens_passes_through_all_non_thai_kinds() {
    use kham_core::token::{Token, TokenKind};
    let tagger = NeTagger::from_tsv("hello\tPERSON\nกิน\tPLACE\n");
    // Craft one token of each non-Thai kind using the same text so a gazetteer
    // hit would be possible if the tagger incorrectly matched them.
    let kinds = [
        TokenKind::Latin,
        TokenKind::Number,
        TokenKind::Punctuation,
        TokenKind::Emoji,
        TokenKind::Whitespace,
        TokenKind::Unknown,
    ];
    for kind in kinds {
        let tok = Token::new("hello", 0..5, 0..5, kind);
        let result = tagger.tag_tokens(vec![tok]);
        assert_eq!(
            result[0].kind, kind,
            "kind {:?} should pass through tag_tokens unchanged",
            kind
        );
    }
}

// ── Gap 2: romanization synonym injected for Named tokens ─────────────────────

#[test]
fn ne_token_gets_rtgs_synonym_when_romanization_enabled() {
    use kham_core::romanizer::RomanizationMap;
    use kham_core::stopwords::StopwordSet;

    // Use custom maps so the test does not depend on the built-in tables
    // containing the same word. กิน is a single-token common verb — it will
    // segment as Thai, get relabelled to Named(Place) by the custom tagger,
    // then pick up "kin" from the custom romanization map, exercising the
    // Thai|Named(_) branch in fts.rs.
    let ne = NeTagger::from_tsv("กิน\tPLACE\n");
    let rom = RomanizationMap::from_tsv("กิน\tkin\n");
    let fts = FtsTokenizer::builder()
        .ne_tagger(ne)
        .romanization(rom)
        .stopwords(StopwordSet::from_text(""))
        .build();
    let tokens = fts.segment_for_fts("กิน");
    let t = tokens.iter().find(|t| t.text == "กิน");
    assert!(t.is_some(), "expected 'กิน' token");
    let t = t.unwrap();
    assert_eq!(
        t.kind,
        TokenKind::Named(NamedEntityKind::Place),
        "กิน should be Named(Place) via custom tagger"
    );
    assert!(
        t.synonyms.iter().any(|s| s == "kin"),
        "expected RTGS synonym 'kin' for Named token, got {:?}",
        t.synonyms
    );
}

// ── Gap 3: NE token text survives into lexemes() ─────────────────────────────

#[test]
fn ne_token_text_appears_in_lexemes() {
    let fts = FtsTokenizer::new();
    // ไทย is PLACE in the gazetteer and is not a stopword
    let lexemes = fts.lexemes("ไทย");
    assert!(
        lexemes.iter().any(|l| l == "ไทย"),
        "NE token 'ไทย' should appear in lexemes(), got {:?}",
        lexemes
    );
}

// ── Gap 4: NE token can also be a stopword ────────────────────────────────────

#[test]
fn ne_token_is_stop_when_in_stopword_list() {
    use kham_core::stopwords::StopwordSet;

    // Force ไทย into the stopword list to test that is_stop and ne can both be
    // set simultaneously — the pipeline should not short-circuit on one or the other.
    let stops = StopwordSet::from_text("ไทย\n");
    let fts = FtsTokenizer::builder().stopwords(stops).build();
    let tokens = fts.segment_for_fts("ไทย");
    let t = tokens.iter().find(|t| t.text == "ไทย");
    assert!(t.is_some(), "expected 'ไทย' token");
    let t = t.unwrap();
    assert_eq!(
        t.kind,
        TokenKind::Named(NamedEntityKind::Place),
        "kind should still be Named(Place) even when is_stop"
    );
    assert_eq!(t.ne, Some(NamedEntityKind::Place), "ne field should be set");
    assert!(
        t.is_stop,
        "is_stop should be true when word is in stopword list"
    );
}
