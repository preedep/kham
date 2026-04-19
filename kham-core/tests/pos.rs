use kham_core::fts::FtsTokenizer;
use kham_core::pos::{PosTag, PosTagger};

#[test]
fn builtin_tagger_non_empty() {
    let t = PosTagger::builtin();
    assert!(t.len() > 50);
}

#[test]
fn verb_noun_adj_lookup() {
    let t = PosTagger::builtin();
    assert_eq!(t.tag("กิน"), Some(PosTag::Verb));
    assert_eq!(t.tag("ข้าว"), Some(PosTag::Noun));
    assert_eq!(t.tag("ดี"), Some(PosTag::Adj));
    assert_eq!(t.tag("และ"), Some(PosTag::Conjunction));
    assert_eq!(t.tag("ได้"), Some(PosTag::Auxiliary));
    assert_eq!(t.tag("นี้"), Some(PosTag::Determiner));
    assert_eq!(t.tag("ใน"), Some(PosTag::Preposition));
    assert_eq!(t.tag("ครับ"), Some(PosTag::Particle));
    assert_eq!(t.tag("ไทย"), Some(PosTag::ProperNoun));
    assert_eq!(t.tag("หนึ่ง"), Some(PosTag::Numeral));
    assert_eq!(t.tag("ตัว"), Some(PosTag::Classifier));
    assert_eq!(t.tag("ฉัน"), Some(PosTag::Pronoun));
}

#[test]
fn oov_returns_none() {
    let t = PosTagger::builtin();
    assert_eq!(t.tag("เปปซี่"), None);
    assert_eq!(t.tag(""), None);
}

#[test]
fn all_tag_roundtrips() {
    use PosTag::*;
    for tag in [
        Noun,
        Verb,
        Adj,
        Adv,
        Particle,
        ProperNoun,
        Pronoun,
        Numeral,
        Classifier,
        Conjunction,
        Auxiliary,
        Determiner,
        Preposition,
    ] {
        assert_eq!(PosTag::from_tag(tag.as_tag()), Some(tag));
    }
}

#[test]
fn fts_token_has_pos_for_known_thai_word() {
    let fts = FtsTokenizer::new();
    let tokens = fts.segment_for_fts("กินข้าว");
    let gin = tokens.iter().find(|t| t.text == "กิน");
    assert!(gin.is_some(), "expected 'กิน' token");
    assert_eq!(gin.unwrap().pos, Some(PosTag::Verb));

    let khao = tokens.iter().find(|t| t.text == "ข้าว");
    assert!(khao.is_some(), "expected 'ข้าว' token");
    assert_eq!(khao.unwrap().pos, Some(PosTag::Noun));
}

#[test]
fn fts_token_pos_none_for_oov() {
    let fts = FtsTokenizer::new();
    // OOV Thai word — pos should be None
    let tokens = fts.segment_for_fts("เปปซี่");
    for t in &tokens {
        assert!(t.pos.is_none(), "OOV token '{}' should have no POS", t.text);
    }
}

#[test]
fn fts_builder_custom_pos_tagger() {
    let tagger = PosTagger::from_tsv("กิน\tNOUN\n"); // intentionally wrong, just for test
    let fts = FtsTokenizer::builder()
        .pos_tagger(tagger)
        .stopwords(kham_core::stopwords::StopwordSet::from_text(""))
        .build();
    let tokens = fts.segment_for_fts("กิน");
    let gin = tokens.iter().find(|t| t.text == "กิน");
    if let Some(t) = gin {
        assert_eq!(t.pos, Some(PosTag::Noun)); // custom tagger overrides builtin
    }
}

#[test]
fn latin_and_number_tokens_have_no_pos() {
    let fts = FtsTokenizer::new();
    let tokens = fts.segment_for_fts("hello123");
    for t in &tokens {
        assert!(
            t.pos.is_none(),
            "non-Thai token '{}' should have no POS",
            t.text
        );
    }
}
