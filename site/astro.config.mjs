import { defineConfig } from 'astro/config';

// GitHub Pages project site: https://adnanhussainturki.github.io/servicio/
const SITE = 'https://adnanhussainturki.github.io';
const BASE = '/servicio';

const locales = ['en', 'es', 'fr', 'de', 'ar', 'hi'];

export default defineConfig({
  site: SITE,
  base: BASE,
  trailingSlash: 'ignore',
  i18n: {
    defaultLocale: 'en',
    locales,
    routing: {
      prefixDefaultLocale: false,
    },
  },
  // hreflang alternates are emitted per-page in <head> (see Base.astro) and in
  // a hand-written public/sitemap.xml (multilingual <xhtml:link> entries).
  integrations: [],
});
