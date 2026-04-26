import { ui, defaultLang, type Lang, type UiKey } from './ui';

export function getLangFromUrl(url: URL): Lang {
  const [, first] = url.pathname.split('/');
  if (first === 'th') return 'th';
  return defaultLang;
}

export function useTranslations(lang: Lang) {
  return function t(key: UiKey): string {
    return (ui[lang] as Record<string, string>)[key]
      ?? (ui[defaultLang] as Record<string, string>)[key]
      ?? key;
  };
}

/** Strip the locale prefix to get the bare page path (e.g. /th/demo → /demo). */
export function getPagePath(pathname: string): string {
  if (pathname === '/th' || pathname === '/th/') return '/';
  if (pathname.startsWith('/th/')) return pathname.slice(3);
  return pathname;
}

/** Return the equivalent URL in the other language. */
export function getAlternateUrl(pathname: string, currentLang: Lang): string {
  const pagePath = getPagePath(pathname);
  if (currentLang === 'en') {
    return pagePath === '/' ? '/th/' : `/th${pagePath}`;
  }
  return pagePath || '/';
}

/** Return the locale prefix for link generation. */
export function prefix(lang: Lang): string {
  return lang === 'th' ? '/th' : '';
}
