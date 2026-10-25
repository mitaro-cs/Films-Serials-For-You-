import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';

// `npm run dev`: интерфейс на :5173, API — у `fsfy serve` на :17480.
const target = process.env.FSFY_URL ?? 'http://127.0.0.1:17480';

export default defineConfig({
  plugins: [sveltekit()],
  server: { proxy: { '/api': target, '/img': target } },
  test: { include: ['src/**/*.test.ts'] },
});
