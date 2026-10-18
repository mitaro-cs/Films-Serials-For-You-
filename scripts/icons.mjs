// Значок приложения и сайта из логотипа web/static/logo.svg (белый знак, источник).
// Пишет web/static/favicon.svg, desktop/icon-src/icon.svg и PNG (рисует Chromium из Playwright):
// desktop/icon-src/icon.png 1024, web/static/icon-192/512, apple-touch-icon.
// Запуск: node scripts/icons.mjs, затем `npm run icons` в desktop/.
import { readFileSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const logo = readFileSync(join(root, 'web/static/logo.svg'), 'utf8');
const d = logo.match(/ d="([^"]+)"/)[1];
const [, , w, h] = logo.match(/viewBox="([^"]+)"/)[1].split(' ').map(Number);

// Плитка как у значков macOS: 824 из 1024, скругление 185; знак — по ширине 64 % плитки.
function tile(size = 1024, inset = 100) {
  const t = size - inset * 2;
  const k = (t * 0.64) / w;
  const x = (size - w * k) / 2;
  const y = (size - h * k) / 2;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${size} ${size}"><defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#2a2a2d"/><stop offset="1" stop-color="#050505"/></linearGradient></defs><rect x="${inset}" y="${inset}" width="${t}" height="${t}" rx="${t * 0.2245}" fill="url(#g)"/><path transform="translate(${x.toFixed(1)} ${y.toFixed(1)}) scale(${k.toFixed(4)})" fill="#fff" fill-rule="evenodd" d="${d}"/></svg>\n`;
}

writeFileSync(join(root, 'desktop/icon-src/icon.svg'), tile());
writeFileSync(join(root, 'web/static/favicon.svg'), tile(1024, 0));

const require = createRequire(join(root, 'web/package.json'));
const { chromium } = require('playwright');
const browser = await chromium.launch();
const page = await browser.newPage();
async function png(svg, size, file) {
  await page.setViewportSize({ width: size, height: size });
  await page.setContent(
    `<style>html,body{margin:0;background:transparent}img{display:block;width:${size}px;height:${size}px}</style><img src="data:image/svg+xml;base64,${Buffer.from(svg).toString('base64')}">`,
  );
  await page.locator('img').evaluate((i) => i.decode());
  await page.screenshot({ path: join(root, file), omitBackground: true });
}
await png(tile(), 1024, 'desktop/icon-src/icon.png');
await png(tile(1024, 0), 512, 'web/static/icon-512.png');
await png(tile(1024, 0), 192, 'web/static/icon-192.png');
await png(tile(1024, 0), 180, 'web/static/apple-touch-icon.png');
await browser.close();
