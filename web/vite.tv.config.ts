import { defineConfig } from 'vite';

// ТВ-интерфейс (`web/tv`) собирается отдельно, под Chrome 69 (Samsung Tizen 5.5), и кладётся в
// `static/tv`: SvelteKit копирует его в `build`, оттуда он попадает в программу. Сервер сам
// отдаёт его телевизорам (`web.rs`). `npm run build:tv`.
export default defineConfig({
  root: 'tv',
  base: '/tv/',
  build: {
    outDir: '../static/tv',
    emptyOutDir: true,
    target: 'chrome69',
    cssTarget: 'chrome69',
    modulePreload: false,
    sourcemap: false,
  },
});
