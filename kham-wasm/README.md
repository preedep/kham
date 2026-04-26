# kham-wasm

WebAssembly bindings for the kham Thai word segmentation engine, built with wasm-bindgen.

## Install

```bash
npm install kham-wasm
```

## Usage

```js
import init, { segment, segment_tokens } from "kham-wasm";

await init();

// Simple segmentation — returns string[]
const words = segment("กินข้าวกับปลา");
console.log(words); // ["กิน", "ข้าว", "กับ", "ปลา"]

// Rich tokens — returns Token objects with offsets and kind
const tokens = segment_tokens("ธนาคาร100แห่ง");
for (const t of tokens) {
    console.log(t.text, t.char_start, t.char_end, t.kind);
}
// ธนาคาร  0  6  Thai
// 100     6  9  Number
// แห่ง    9  13 Thai
```

## Token fields

| Field | Type | Description |
|---|---|---|
| `text` | `string` | Token text |
| `byte_start` / `byte_end` | `number` | Byte offsets into the UTF-8 encoded input |
| `char_start` / `char_end` | `number` | Unicode scalar-value offsets |
| `kind` | `string` | One of `Thai`, `Latin`, `Number`, `Punctuation`, `Emoji`, `Whitespace`, `Unknown`, `Named` |

## Build from source

```bash
wasm-pack build kham-wasm --target web
# → kham-wasm/pkg/
```

For Node.js:

```bash
wasm-pack build kham-wasm --target nodejs
```
