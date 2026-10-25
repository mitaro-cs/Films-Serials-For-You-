import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vitest/config';

// `npm run dev`: интерфейс на :5173, API — у `fsfy serve` на :17480.
const target = process.env.FSFY_URL ?? 'http://127.0.0.1:17480';

export default defineConfig({
  // SvelteKit 3: настройки — здесь, `svelte.config.js` больше не читается.
  plugins: [
    sveltekit({
      preprocess: vitePreprocess(),
      // В SvelteKit 3 `$lib` по умолчанию нет (замена — `#lib` в package.json); оставляем свой.
      alias: { $lib: 'src/lib' },
      adapter: adapter({ fallback: 'index.html' }),
      csp: { mode: 'hash', directives: { 'script-src': ['self'] } },
    }),
  ],
  server: { proxy: { '/api': target, '/img': target } },
  test: { include: ['src/**/*.test.ts'] },
});
