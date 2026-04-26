export const languages = { en: 'English', th: 'ภาษาไทย' } as const;
export type Lang = keyof typeof languages;
export const defaultLang: Lang = 'en';

export const ui = {
  en: {
    // Nav
    'nav.getting-started': 'Getting Started',
    'nav.demo': 'Demo',
    'nav.api': 'API',
    // Footer
    'footer.tagline': 'Thai word segmentation in Rust',
    'footer.getting-started': 'Getting Started',
    'footer.license': 'MIT OR Apache-2.0',
    // Landing — hero
    'home.hero.title': 'Fast Thai Word Segmentation in Rust',
    'home.hero.desc': 'kham is a batteries-included Thai NLP engine — zero external dependencies, no_std core, and ready for Rust, WebAssembly, Python, C, PostgreSQL, and SQLite.',
    'home.hero.cta.docs': 'Docs',
    'home.hero.cta.demo': 'Live Demo',
    // Landing — features
    'home.features.title': 'Everything you need for Thai NLP',
    'home.features.subtitle': 'A single library for the full pipeline — from raw text to structured tokens with semantic metadata.',
    'home.features.fast.title': 'Fast',
    'home.features.fast.body': 'Maximal Matching on a compressed DAWG dictionary. Outperforms PyThaiNLP newmm on throughput while matching accuracy.',
    'home.features.multi.title': 'Multi-target',
    'home.features.multi.body': 'One core, many targets: Rust crate, WebAssembly, Python (PyO3), C FFI, CLI, PostgreSQL FTS parser, SQLite FTS5 tokenizer.',
    'home.features.nostd.title': 'no_std core',
    'home.features.nostd.body': 'kham-core is pure Rust with no_std + alloc. Runs in embedded, WASM, and any environment without a standard library.',
    'home.features.pipeline.title': 'Full NLP pipeline',
    'home.features.pipeline.body': 'Segmentation, POS tagging, Named Entity Recognition, romanization (RTGS), phonetic codes (lk82 / udom83 / MetaSound), number normalization.',
    // Landing — code section
    'home.code.title': 'Simple, zero-copy API',
    'home.code.body': 'Segment Thai text into tokens with byte and Unicode char spans — suitable for search indexing, NLP pipelines, and binding to any language runtime.',
    'home.code.link': 'Getting started guide →',
    // Landing — demo teaser
    'home.demo.title': 'Try it right now',
    'home.demo.subtitle': 'Powered by WebAssembly — runs entirely in your browser, no server needed.',
    'home.demo.link': 'Open full playground →',
    // Landing — CTA
    'home.cta.title': 'Ready to integrate?',
    'home.cta.body': 'Add kham to your Rust, Python, or Node.js project in minutes.',
    'home.cta.get-started': 'Get started',
    'home.cta.demo': 'Try the live demo',
    // Demo page
    'demo.title': 'Live Demo',
    'demo.desc': 'Thai word segmentation running entirely in your browser via WebAssembly — no server required. Type any Thai text and click Segment, or pick a sample.',
    'demo.kinds.title': 'Token kinds',
    'demo.span.title': 'Span columns',
    'demo.span.chars': 'Unicode scalar-value offsets, suitable for Python / JS str.slice()',
    'demo.span.bytes': 'UTF-8 byte offsets, used internally by Rust / PostgreSQL FTS.',
    'demo.coming.title': 'Coming in v0.3',
    // Getting Started
    'gs.title': 'Getting Started',
    'gs.subtitle': 'kham v0.5.0 — pick your target and be up and running in minutes.',
    'gs.install': 'Install',
    'gs.usage': 'Usage',
    'gs.full-guide': 'Full integration guide →',
    // API
    'api.title': 'API Reference',
    'api.desc': 'Public API of kham-core — the pure Rust, no_std core library. Full rustdoc is available on docs.rs/kham-core.',
    'api.table.module': 'Module / Type',
    'api.table.path': 'Path',
    'api.table.desc': 'Description',
    'api.back': '← Getting Started',
    'api.rustdoc': 'Full rustdoc on docs.rs ↗',
    // Benchmarks
    'nav.benchmarks': 'Benchmarks',
    // Changelog
    'nav.changelog': 'Changelog',
    // License
    'nav.license': 'License',
  },
  th: {
    // Nav
    'nav.getting-started': 'เริ่มต้นใช้งาน',
    'nav.demo': 'ทดลองใช้งาน',
    'nav.api': 'อ้างอิง API',
    'nav.benchmarks': 'ประสิทธิภาพ',
    'nav.changelog': 'บันทึกการเปลี่ยนแปลง',
    'nav.license': 'ใบอนุญาต',
    // Footer
    'footer.tagline': 'ตัดคำภาษาไทยด้วย Rust',
    'footer.getting-started': 'เริ่มต้นใช้งาน',
    'footer.license': 'MIT หรือ Apache-2.0',
    // Landing — hero
    'home.hero.title': 'ตัดคำภาษาไทยความเร็วสูงด้วย Rust',
    'home.hero.desc': 'kham เป็นไลบรารีสำหรับ NLP ภาษาไทยพร้อมใช้งาน ไม่มี dependency ภายนอก มี core แบบ no_std รองรับ Rust, WebAssembly, Python, C, PostgreSQL และ SQLite',
    'home.hero.cta.docs': 'เอกสาร',
    'home.hero.cta.demo': 'ทดลองใช้งาน',
    // Landing — features
    'home.features.title': 'ครอบคลุมทุกความต้องการของ Thai NLP',
    'home.features.subtitle': 'ไลบรารีเดียวสำหรับ pipeline ทั้งหมด ตั้งแต่ข้อความดิบจนถึง token พร้อม metadata',
    'home.features.fast.title': 'รวดเร็ว',
    'home.features.fast.body': 'Maximal Matching บน DAWG dictionary ที่บีบอัดแล้ว ทำงานเร็วกว่า PyThaiNLP newmm โดยยังรักษาความแม่นยำไว้',
    'home.features.multi.title': 'รองรับหลายแพลตฟอร์ม',
    'home.features.multi.body': 'หนึ่ง core หลายเป้าหมาย: Rust crate, WebAssembly, Python (PyO3), C FFI, CLI, PostgreSQL FTS parser และ SQLite FTS5 tokenizer',
    'home.features.nostd.title': 'no_std core',
    'home.features.nostd.body': 'kham-core เป็น pure Rust แบบ no_std + alloc ทำงานได้ใน embedded, WASM และทุกสภาพแวดล้อมที่ไม่มี standard library',
    'home.features.pipeline.title': 'pipeline NLP ครบชุด',
    'home.features.pipeline.body': 'ตัดคำ, POS tagging, Named Entity Recognition, romanization (RTGS), phonetic codes (lk82/udom83/MetaSound) และ normalize ตัวเลข',
    // Landing — code section
    'home.code.title': 'API ที่เรียบง่าย ไม่คัดลอกข้อมูล',
    'home.code.body': 'ตัดคำภาษาไทยพร้อม byte span และ char span สำหรับ search indexing, NLP pipeline และการเชื่อมต่อกับทุกภาษา',
    'home.code.link': 'คู่มือเริ่มต้นใช้งาน →',
    // Landing — demo teaser
    'home.demo.title': 'ทดลองใช้งานเดี๋ยวนี้',
    'home.demo.subtitle': 'ขับเคลื่อนด้วย WebAssembly — ทำงานในเบราว์เซอร์ทั้งหมด ไม่ต้องใช้เซิร์ฟเวอร์',
    'home.demo.link': 'เปิด playground เต็มรูปแบบ →',
    // Landing — CTA
    'home.cta.title': 'พร้อมนำไปใช้งานแล้วหรือยัง?',
    'home.cta.body': 'เพิ่ม kham ใน Rust, Python หรือ Node.js ของคุณได้ภายในไม่กี่นาที',
    'home.cta.get-started': 'เริ่มต้น',
    'home.cta.demo': 'ทดลองใช้งาน',
    // Demo page
    'demo.title': 'ทดลองใช้งาน',
    'demo.desc': 'ตัดคำภาษาไทยทำงานในเบราว์เซอร์ผ่าน WebAssembly ไม่ต้องใช้เซิร์ฟเวอร์ พิมพ์ข้อความภาษาไทยและคลิก ตัดคำ หรือเลือกตัวอย่าง',
    'demo.kinds.title': 'ประเภทของ token',
    'demo.span.title': 'คอลัมน์ span',
    'demo.span.chars': 'Unicode scalar-value offsets เหมาะสำหรับ Python / JS str.slice()',
    'demo.span.bytes': 'UTF-8 byte offsets ใช้ภายใน Rust / PostgreSQL FTS',
    'demo.coming.title': 'เพิ่มเติมใน v0.3',
    // Getting Started
    'gs.title': 'เริ่มต้นใช้งาน',
    'gs.subtitle': 'kham v0.5.0 — เลือกแพลตฟอร์มที่ต้องการและเริ่มใช้งานได้ภายในไม่กี่นาที',
    'gs.install': 'ติดตั้ง',
    'gs.usage': 'การใช้งาน',
    'gs.full-guide': 'คู่มือการใช้งานแบบเต็ม →',
    // API
    'api.title': 'อ้างอิง API',
    'api.desc': 'Public API ของ kham-core — ไลบรารี pure Rust แบบ no_std เอกสารฉบับเต็มอยู่ที่ docs.rs/kham-core',
    'api.table.module': 'โมดูล / ประเภท',
    'api.table.path': 'Path',
    'api.table.desc': 'คำอธิบาย',
    'api.back': '← เริ่มต้นใช้งาน',
    'api.rustdoc': 'rustdoc ฉบับเต็มที่ docs.rs ↗',
  },
} as const;

export type UiKey = keyof typeof ui[typeof defaultLang];
