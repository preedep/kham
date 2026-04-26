---
name: kham-web
description: Build and maintain the kham.io Astro website. Use when scaffolding kham-web/, adding pages, building the live demo component (kham-wasm in browser), writing Astro components, configuring Tailwind, setting up CI/CD for the site, or deploying to GitHub Pages / Cloudflare Pages.
---

# kham-web — Astro Website for kham.io

## Project Location

`kham-web/` inside the kham monorepo root. Not a Rust crate — no Cargo.toml entry.

## Stack

- **Astro** — static site generator (content collections for docs/changelog)
- **Tailwind CSS** — utility-first styling
- **kham-wasm** — Thai segmentation running live in the browser (no backend)
- **TypeScript** — all components and scripts

## Bootstrap

```bash
cd kham-web
npm install
npm run dev          # dev server at http://localhost:4321
npm run build        # production build → dist/
npm run preview      # preview production build
```

## WASM Integration

Build kham-wasm first, then reference from kham-web:

```bash
wasm-pack build kham-wasm --target web --release
# Output: kham-wasm/pkg/
```

In Astro components import via relative path during development:
```ts
import init, { segment_tokens } from '../../kham-wasm/pkg/kham_wasm.js';
```

For production, import from npm package `kham-wasm` once published.

## Pages

| Route | File | Purpose |
|-------|------|---------|
| `/` | `src/pages/index.astro` | Landing page — hero, badges, live demo teaser |
| `/demo` | `src/pages/demo.astro` | Full interactive playground |
| `/getting-started` | `src/pages/getting-started.astro` | Install + quickstart per target |
| `/api` | `src/pages/api/index.astro` | API reference (kham-core public API) |
| `/integrations` | `src/pages/integrations/[slug].astro` | PG / SQLite / Python / WASM guides |
| `/benchmarks` | `src/pages/benchmarks.astro` | Performance + accuracy numbers |
| `/changelog` | `src/pages/changelog.astro` | Per-version release notes |
| `/license` | `src/pages/license.astro` | License text + corpus attributions |

## Key Components

- `BadgeRow.astro` — crates.io / docs.rs / CI / license badges
- `LiveDemo.astro` — WASM-powered Thai text input + token output; loads `/wasm/kham_wasm.js` via `is:inline` dynamic import; supports `compact` prop for landing-page teaser
- `CodeBlock.astro` — syntax-highlighted code snippets with copy button; uses Astro's built-in `<Code>` (Shiki, no extra packages); accepts `tabs` array or single `code/lang/label`
- `NavBar.astro` / `Footer.astro`

## CodeBlock usage

```astro
<!-- Single snippet -->
<CodeBlock code="cargo add kham-core" lang="bash" label="shell" />

<!-- Multi-tab -->
<CodeBlock tabs={[
  { label: 'pip', lang: 'bash', code: 'pip install kham' },
  { label: 'uv',  lang: 'bash', code: 'uv add kham' },
]} />
```

## WASM integration (Phase 2)

- `npm run setup:wasm` copies `kham-wasm/pkg/{kham_wasm.js,kham_wasm_bg.wasm}` → `public/wasm/`; wired as `predev` / `prebuild` hook
- `public/wasm/` is gitignored (generated artifact)
- In CI (`web.yml`) wasm-pack builds kham-wasm before `npm run build`
- Client scripts import via `import('/wasm/kham_wasm.js')` inside `<script is:inline>` — Vite does not process this import; browser fetches it at runtime

## CI / Deployment

- GitHub Actions workflow: `.github/workflows/web.yml`
- Trigger: push to `main` touching `kham-web/**` or `kham-wasm/**`
- Steps: build kham-wasm → `npm run build` → deploy to GitHub Pages (or Cloudflare Pages)

## Conventions

- Tailwind only — no custom CSS files
- All Thai text in demos must be valid UTF-8
- WASM loaded lazily (dynamic import) so the page is not blocked
- No server-side rendering — `output: 'static'` in astro.config.mjs
