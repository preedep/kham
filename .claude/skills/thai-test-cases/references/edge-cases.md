# Thai Segmentation Edge Cases Catalog

## Category 1: Normalization Issues

| Input | Issue | Expected behavior |
|---|---|---|
| `"เเม่"` (double เ) | User typed เ twice instead of แ | Normalizer should fix → `"แม่"` |
| `"ก้้อน"` (double ้) | Duplicate tone mark | Keep first, remove duplicate |
| `"สวัสดี\u200B"` (ZWSP) | Zero-width space at end | Strip ZWSP |
| `"\u0E40\u0E01"` (เก) | Leading vowel as separate codepoint | TCC must group: เ+ก = 1 TCC |

## Category 2: Ambiguous Segmentation

| Input | Option A | Option B | Preferred |
|---|---|---|---|
| ตากลม | ตา+กลม | ตาก+ลม | Context-dependent |
| คนขับรถ | คนขับ+รถ | คน+ขับรถ | คน+ขับ+รถ |
| ทำให้เสีย | ทำ+ให้+เสีย | ทำให้+เสีย | ทำให้+เสีย |
| ตำรวจ | ตำ+รวจ | ตำรวจ | ตำรวจ (dict word) |

## Category 3: Mixed Script

| Input | Expected tokens |
|---|---|
| `"iPhone15Pro"` | `["iPhone15Pro"]` (single Latin+Num) |
| `"ซื้อiPhone"` | `["ซื้อ", "iPhone"]` |
| `"100บาท"` | `["100", "บาท"]` |
| `"ราคา$50"` | `["ราคา", "$", "50"]` |
| `"email@test.com"` | `["email@test.com"]` (preserve URL) |

## Category 4: OOV (Out-of-Vocabulary)

| Input | Expected behavior |
|---|---|
| `"สตรอเบอรี่"` (not in dict) | Keep as single unknown token if possible |
| `"บล็อกเชน"` | Merge unknown TCCs into one token |
| `"ดิสรัปชั่น"` | Should not split into single chars |

## Category 5: Special Characters

| Input | Expected behavior |
|---|---|
| `"(สวัสดี)"` | `["(", "สวัสดี", ")"]` |
| `"สวัสดี..."` | `["สวัสดี", "..."]` |
| `"ราคา 100 บาท"` | `["ราคา", " ", "100", " ", "บาท"]` (keep whitespace) |
| `"#แฮชแท็ก"` | `["#", "แฮชแท็ก"]` |

## Category 6: Performance Edge Cases

| Input | Risk | Mitigation |
|---|---|---|
| 10,000 chars, no spaces | DAG explosion | Safe mode threshold |
| All unknown chars | Degenerate to char-by-char | TCC merge fallback |
| Single repeated char × 1000 | Pathological matching | Early termination |
