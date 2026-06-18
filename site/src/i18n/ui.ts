import en from './en.json';
import es from './es.json';
import fr from './fr.json';
import de from './de.json';
import ar from './ar.json';
import hi from './hi.json';

export const defaultLocale = 'en' as const;

export type Locale = 'en' | 'es' | 'fr' | 'de' | 'ar' | 'hi';

export const locales: Locale[] = ['en', 'es', 'fr', 'de', 'ar', 'hi'];

/** Display name (endonym), text direction, and OG locale per language. */
export const localeMeta: Record<Locale, { name: string; dir: 'ltr' | 'rtl'; og: string }> = {
  en: { name: 'English', dir: 'ltr', og: 'en_US' },
  es: { name: 'Español', dir: 'ltr', og: 'es_ES' },
  fr: { name: 'Français', dir: 'ltr', og: 'fr_FR' },
  de: { name: 'Deutsch', dir: 'ltr', og: 'de_DE' },
  ar: { name: 'العربية', dir: 'rtl', og: 'ar_AR' },
  hi: { name: 'हिन्दी', dir: 'ltr', og: 'hi_IN' },
};

const dictionaries: Record<Locale, Record<string, unknown>> = { en, es, fr, de, ar, hi };

/** Resolve a dotted key (e.g. "hero.title") for a locale, falling back to English. */
export function useTranslations(locale: Locale) {
  const dict = dictionaries[locale];
  const fallback = dictionaries[defaultLocale];
  return function t(key: string): string {
    const lookup = (d: Record<string, unknown>): string | undefined =>
      key.split('.').reduce<unknown>((o, k) => (o as Record<string, unknown>)?.[k], d) as
        | string
        | undefined;
    return lookup(dict) ?? lookup(fallback) ?? key;
  };
}

/** A feature card list lives under the "features.items" array key. */
export function useList(locale: Locale, key: string): { title: string; body: string }[] {
  const dict = dictionaries[locale];
  const fallback = dictionaries[defaultLocale];
  const lookup = (d: Record<string, unknown>): unknown =>
    key.split('.').reduce<unknown>((o, k) => (o as Record<string, unknown>)?.[k], d);
  return (lookup(dict) ?? lookup(fallback) ?? []) as { title: string; body: string }[];
}

/** Build a locale-aware path honouring Astro's base + default-no-prefix routing. */
export function localizedPath(locale: Locale, base: string): string {
  const clean = base.replace(/\/$/, '');
  return locale === defaultLocale ? `${clean}/` : `${clean}/${locale}/`;
}
