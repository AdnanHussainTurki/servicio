# Servicio — marketing site

The public landing page for Servicio, built with [Astro](https://astro.build) and
deployed to GitHub Pages at **https://adnanhussainturki.github.io/servicio/**.

## Highlights

- **Single landing page** — hero, problem, features, screenshots, architecture, install.
- **6 languages** — English (default, no prefix), Spanish, French, German, Arabic (RTL),
  Hindi. Strings live in `src/i18n/<locale>.json`; routing is Astro's built-in i18n with
  `prefixDefaultLocale: false`.
- **SEO-first** — per-locale `<title>`/description, canonical URLs, full `hreflang`
  alternates (+ `x-default`), Open Graph + Twitter cards, `SoftwareApplication` JSON-LD,
  a multilingual `public/sitemap.xml`, and `robots.txt`. Static output for fast LCP.
- **Control-room aesthetic** — dark by default with a persisted light toggle, mirrors the
  app's telemetry-HUD look. RTL handled via CSS logical properties.

## Develop

```bash
cd site
npm install
npm run dev        # http://localhost:4321/servicio/
npm run build      # static output → dist/
npm run preview    # serve the built site
```

## Deploy

`.github/workflows/pages.yml` builds this folder and publishes it on every push to
`main` that touches `site/**`. **One-time setup:** in the repo, go to
**Settings → Pages → Build and deployment → Source** and select **GitHub Actions**.

## Editing content

- **Copy** — edit the JSON files in `src/i18n/`. Keep the key structure identical across
  locales; missing keys fall back to English.
- **Layout/SEO** — `src/layouts/Base.astro` (head, meta, JSON-LD).
- **Sections** — `src/components/*.astro`, composed in `src/components/Landing.astro`.
- **Adding a locale** — add it to `locales`/`localeMeta` in `src/i18n/ui.ts`, drop in a
  `<locale>.json`, and add a `<url>` block to `public/sitemap.xml`.
- **Screenshots** — sourced from the repo's `docs/images/`, copied into `public/images/`.
