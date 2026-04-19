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
