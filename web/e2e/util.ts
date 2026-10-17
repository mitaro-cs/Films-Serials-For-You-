import { expect, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

export const env = (): { baseURL: string; publicURL: string; ids: Record<string, number> } =>
  JSON.parse(readFileSync(join(import.meta.dirname, '.env.json'), 'utf8'));

export const url = (path: string) => env().baseURL + path;

/**
 * Вёрстка без пересечений:
 * - страница не шире экрана (нет горизонтальной прокрутки);
 * - ни одна строка текста не наезжает на другую и на кнопку/поле;
 * - кнопки и поля не наезжают друг на друга;
 * - ничего не вылезает за правый край, кроме лент с прокруткой.
 * Нижняя панель телефона (fixed внизу) с прокручиваемым содержимым не сравнивается.
 */
export async function expectClean(page: Page, what: string) {
  await page.waitForLoadState('networkidle');
  await page.evaluate(() => document.fonts.ready);
  const problems = await page.evaluate(() => {
    const out: string[] = [];
    const W = document.documentElement.clientWidth;
    if (document.documentElement.scrollWidth > W + 1) {
      // Кто распирает: самый глубокий элемент за правым краем без прокручиваемого предка.
      const culprits = [...document.body.querySelectorAll('*')].filter((e) => {
        if (e.getBoundingClientRect().right <= W + 1) return false;
        for (let p = e.parentElement; p && p !== document.body; p = p.parentElement) {
          if (getComputedStyle(p).overflowX !== 'visible') return false;
        }
        return ![...e.children].some((c) => c.getBoundingClientRect().right > W + 1);
      });
      const who = culprits.slice(0, 3).map((e) => `<${e.tagName.toLowerCase()} class="${e.getAttribute('class') ?? ''}">`);
      out.push(`страница шире экрана: ${document.documentElement.scrollWidth} > ${W} (${who.join(', ')})`);
    }

    const visible = (el: Element) => {
      const s = getComputedStyle(el);
      return s.visibility !== 'hidden' && s.display !== 'none' && Number(s.opacity) > 0.05;
    };
    const fixedOf = (el: Element | null): Element | null => {
      for (let e = el; e; e = e.parentElement) {
        const p = getComputedStyle(e).position;
        if (p === 'fixed' || p === 'sticky') return e;
      }
      return null;
    };
    // Обрезка прокручиваемым или overflow:hidden предком — видимая часть.
    const clip = (el: Element, r: DOMRect) => {
      let { left, top, right, bottom } = r;
      for (let e = el.parentElement; e; e = e.parentElement) {
        const s = getComputedStyle(e);
        if (s.overflowX !== 'visible' || s.overflowY !== 'visible') {
          const c = e.getBoundingClientRect();
          left = Math.max(left, c.left);
          right = Math.min(right, c.right);
          top = Math.max(top, c.top);
          bottom = Math.min(bottom, c.bottom);
        }
      }
      return { left, top, right, bottom };
    };
    const scrollsX = (el: Element) => {
      for (let e = el.parentElement; e; e = e.parentElement) {
        const s = getComputedStyle(e).overflowX;
        if (s === 'auto' || s === 'scroll' || s === 'hidden' || s === 'clip') return true;
      }
      return false;
    };
    const name = (el: Element) => {
      const t = (el.textContent ?? '').trim().replace(/\s+/g, ' ').slice(0, 40);
      return `<${el.tagName.toLowerCase()}${el.className && typeof el.className === 'string' ? '.' + el.className.split(' ')[0] : ''}> «${t}»`;
    };

    type Box = { el: Element; r: { left: number; top: number; right: number; bottom: number }; kind: 'text' | 'ctl' };
    const boxes: Box[] = [];
    // Строки текста — по прямоугольникам самих текстовых узлов.
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    for (let n = walker.nextNode(); n; n = walker.nextNode()) {
      if (!n.textContent?.trim()) continue;
      const el = n.parentElement!;
      if (!visible(el) || el.closest('[aria-hidden="true"], svg, script, style')) continue;
      const range = document.createRange();
      range.selectNodeContents(n);
      for (const r of range.getClientRects()) {
        if (r.width < 1 || r.height < 1) continue;
        const c = clip(el, r);
        if (c.right - c.left < 1 || c.bottom - c.top < 1) continue;
        boxes.push({ el, r: c, kind: 'text' });
        if (r.right > W + 1 && !scrollsX(el)) out.push(`за правым краем: ${name(el)} (${Math.round(r.right)} > ${W})`);
      }
    }
    for (const el of document.querySelectorAll('button, a.btn, a.chip, input, select, textarea, [role="tab"]')) {
      if (!visible(el)) continue;
      const r = el.getBoundingClientRect();
      if (r.width < 1 || r.height < 1) continue;
      const c = clip(el, r);
      if (c.right - c.left < 1 || c.bottom - c.top < 1) continue;
      boxes.push({ el, r: c, kind: 'ctl' });
      if (r.right > W + 1 && !scrollsX(el)) out.push(`за правым краем: ${name(el)}`);
    }
    const H = innerHeight;
    for (let i = 0; i < boxes.length; i++) {
      for (let j = i + 1; j < boxes.length; j++) {
        const a = boxes[i];
        const b = boxes[j];
        if (a.el === b.el || a.el.contains(b.el) || b.el.contains(a.el)) continue;
        const fa = fixedOf(a.el);
        const fb = fixedOf(b.el);
        // Нижние плавающие панели и прокручиваемое содержимое: пересечение зависит от прокрутки.
        if (fa !== fb && ((fa && fa.getBoundingClientRect().top > H / 2) || (fb && fb.getBoundingClientRect().top > H / 2))) continue;
        // Всплывающее меню поверх страницы — задумано.
        if ((fa && fa !== fb && getComputedStyle(fa).zIndex > '60') || (fb && fa !== fb && getComputedStyle(fb).zIndex > '60')) continue;
        const w = Math.min(a.r.right, b.r.right) - Math.max(a.r.left, b.r.left);
        const h = Math.min(a.r.bottom, b.r.bottom) - Math.max(a.r.top, b.r.top);
        if (w > 1.5 && h > 1.5) out.push(`пересекаются: ${name(a.el)} и ${name(b.el)} (${Math.round(w)}×${Math.round(h)})`);
      }
    }
    return [...new Set(out)].slice(0, 15);
  });
  expect(problems, `${what}: вёрстка`).toEqual([]);
}

/** Статусы демо-данных как после setup: тесты не зависят друг от друга. */
export async function resetStatuses() {
  const { baseURL, ids } = env();
  const { request } = await import('@playwright/test');
  const api = await request.newContext({ baseURL, storageState: join(import.meta.dirname, '.state.json'), extraHTTPHeaders: { 'x-fsfy': '1' } });
  const set = async (t: string, list: boolean, watched: boolean) => {
    await api.put(`/api/titles/${ids[t]}/watched`, { data: { value: watched } });
    await api.put(`/api/list/${ids[t]}`, { data: { value: list } });
  };
  await set('Амели', false, false);
  await set('Дюна', true, false);
  await set('Интерстеллар', true, false);
  await set('Ла-Ла Ленд', false, true);
  await set('Паразиты', false, true);
  await api.dispose();
}
