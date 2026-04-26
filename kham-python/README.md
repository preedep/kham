# kham-python

Python bindings for the kham Thai word segmentation engine, built with PyO3 and maturin.

## Install

```bash
pip install kham
```

## Usage

```python
import kham

# Simple segmentation — returns a list of strings
tokens = kham.segment("กินข้าวกับปลา")
print(tokens)  # ['กิน', 'ข้าว', 'กับ', 'ปลา']

# Rich tokens — returns Token objects with offsets and kind
tokens = kham.segment_tokens("ธนาคาร100แห่ง")
for t in tokens:
    print(t.text, t.char_start, t.char_end, t.kind)
# ธนาคาร  0  6  Thai
# 100     6  9  Number
# แห่ง    9  13 Thai
```

## Token fields

| Field | Type | Description |
|---|---|---|
| `text` | `str` | Token text |
| `byte_start` / `byte_end` | `int` | Byte offsets into the UTF-8 encoded input |
| `char_start` / `char_end` | `int` | Unicode scalar-value offsets (suitable for Python string slicing) |
| `kind` | `str` | One of `Thai`, `Latin`, `Number`, `Punctuation`, `Emoji`, `Whitespace`, `Unknown`, `Named` |

## Build from source

```bash
pip install maturin
maturin develop -m kham-python/Cargo.toml

# Run tests
pytest kham-python/tests/ -v
```
