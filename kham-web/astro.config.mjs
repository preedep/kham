import { defineConfig } from 'astro/config';
import tailwindcss from '@tailwindcss/vite';
import sitemap from '@astrojs/sitemap';

export default defineConfig({
  output: 'static',
  site: 'https://kham.io',
  integrations: [sitemap({ i18n: { defaultLocale: 'en', locales: { en: 'en', th: 'th' } } })],
  i18n: {
    defaultLocale: 'en',
    locales: ['en', 'th'],
    routing: { prefixDefaultLocale: false },
  },
  vite: {
    plugins: [tailwindcss()],
  },
});
