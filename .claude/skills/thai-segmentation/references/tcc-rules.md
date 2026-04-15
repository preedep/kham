# Thai Character Cluster (TCC) Rules Reference

Source: Theeramunkong et al. 2000 + PyThaiNLP enhanced rules

## Unicode Ranges

```
THAI_CONSONANTS:  \u0E01-\u0E2E  (ก-ฮ)
THAI_VOWELS_UPPER: \u0E31, \u0E34-\u0E3A  (อั, อิ-อฺ)
THAI_VOWELS_LEAD:  \u0E40-\u0E44  (เ แ โ ไ ใ)
THAI_VOWELS_FOLLOW: \u0E30, \u0E32-\u0E33  (อะ อา อำ)
THAI_TONE_MARKS:   \u0E48-\u0E4B  (อ่ อ้ อ๊ อ๋)
THAI_THANTHAKAT:   \u0E4C  (อ์)
THAI_NIKHAHIT:     \u0E4D  (อํ)
THAI_DIGITS:       \u0E50-\u0E59  (๐-๙)
```

## TCC Pattern (simplified regex)

```
TCC = LEAD? CONSONANT UPPER* TONE? (THANTHAKAT | FOLLOW | NIKHAHIT)?
    | NON_THAI_CHAR+
```

Expanded:
1. Optional leading vowel: เ, แ, โ, ไ, ใ
2. One consonant (required)
3. Zero or more upper vowels/marks: อิ, อี, อึ, อื, อุ, อู, อั, อฺ
4. Optional tone mark: ่, ้, ๊, ๋
5. Optional ending: ์ (thanthakat), อะ, อา, อำ, อํ (nikhahit)

## Edge Cases

- ฤ, ฦ are special vowels that act as standalone syllables
- Double เ: แ = เ+เ but treated as single leading vowel
- Consonant clusters: กร, กล, etc. — second consonant starts new TCC
- Sara Am decomposition: อำ can be อํ+อา in some Unicode forms
- Repeated tone marks (malformed text): keep first, discard rest
