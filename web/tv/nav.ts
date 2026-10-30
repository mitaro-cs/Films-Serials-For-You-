// Управление пультом: стрелки двигают фокус к ближайшему элементу `.f` в этом направлении,
// OK нажимает, «Назад» - на шаг назад. Без внешних библиотек.

export type Dir = 'left' | 'right' | 'up' | 'down';

export const KEYS = {
  left: 37,
  up: 38,
  right: 39,
  down: 40,
  enter: 13,
  space: 32,
  esc: 27,
  backspace: 8,
  // Tizen (Samsung): «Назад» - 10009; webOS (LG) - 461.
  tizenBack: 10009,
  webosBack: 461,
  play: 415,
  pause: 19,
  playPause: 10252,
  stop: 413,
  rewind: 412,
  forward: 417,
};

export function dirOf(code: number): Dir | null {
  if (code === KEYS.left) return 'left';
  if (code === KEYS.right) return 'right';
  if (code === KEYS.up) return 'up';
  if (code === KEYS.down) return 'down';
  return null;
}

export function isBack(e: KeyboardEvent): boolean {
  const c = e.keyCode;
  if (c === KEYS.tizenBack || c === KEYS.webosBack || c === KEYS.esc) return true;
  // Backspace - «назад», но не в поле ввода: там он стирает.
  return c === KEYS.backspace && !isTyping();
}

export function isTyping(): boolean {
  const a = document.activeElement;
  return !!a && (a.tagName === 'INPUT' || a.tagName === 'TEXTAREA');
}

function isVisible(el: HTMLElement): boolean {
  const r = el.getBoundingClientRect();
  return r.width > 0 && r.height > 0;
}

export function focusables(root: ParentNode = document): HTMLElement[] {
  const all = Array.prototype.slice.call(root.querySelectorAll('.f')) as HTMLElement[];
  return all.filter(isVisible);
}

export function current(): HTMLElement | null {
  const a = document.activeElement as HTMLElement | null;
  return a && a.classList && a.classList.contains('f') ? a : null;
}

/** Показать элемент: ряд едет по горизонтали, страница - по вертикали. */
export function reveal(el: HTMLElement) {
  const track = el.closest('.track') as HTMLElement | null;
  if (track) {
    const r = el.getBoundingClientRect();
    const t = track.getBoundingClientRect();
    const want = track.scrollLeft + (r.left - t.left) - (t.width - r.width) / 2;
    track.scrollLeft = Math.max(0, want);
  }
  const block = (el.closest('.sec') || el) as HTMLElement;
  const b = block.getBoundingClientRect();
  const h = window.innerHeight;
  if (el.closest('.hero') || el.closest('.bar')) {
    window.scrollTo(0, 0);
  } else if (b.top < h * 0.12 || b.bottom > h * 0.97) {
    window.scrollTo(0, Math.max(0, window.pageYOffset + b.top - h * 0.18));
  }
}

export function focusEl(el: HTMLElement | null) {
  if (!el) return;
  // Ссылки без href сами в фокус не встают.
  if (!el.hasAttribute('tabindex')) el.setAttribute('tabindex', '-1');
  try {
    el.focus({ preventScroll: true });
  } catch (e) {
    el.focus();
  }
  reveal(el);
}

/** Куда фокус из `from` по направлению: ближе по этой оси, меньше сдвиг по другой. */
export function pick(dir: Dir, from: DOMRect, list: HTMLElement[], skip?: HTMLElement): HTMLElement | null {
  let best: HTMLElement | null = null;
  let bestScore = Infinity;
  const fx = from.left + from.width / 2;
  const fy = from.top + from.height / 2;
  for (let i = 0; i < list.length; i++) {
    const el = list[i];
    if (el === skip) continue;
    const b = el.getBoundingClientRect();
    const cx = b.left + b.width / 2;
    const cy = b.top + b.height / 2;
    let main: number;
    let cross: number;
    if (dir === 'right') {
      if (cx <= fx + 1) continue;
      main = Math.max(0, b.left - from.right);
      cross = rangeGap(b.top, b.bottom, from.top, from.bottom) || Math.abs(cy - fy) * 0.15;
    } else if (dir === 'left') {
      if (cx >= fx - 1) continue;
      main = Math.max(0, from.left - b.right);
      cross = rangeGap(b.top, b.bottom, from.top, from.bottom) || Math.abs(cy - fy) * 0.15;
    } else if (dir === 'down') {
      if (cy <= fy + 1) continue;
      main = Math.max(0, b.top - from.bottom);
      cross = rangeGap(b.left, b.right, from.left, from.right) || Math.abs(cx - fx) * 0.15;
    } else {
      if (cy >= fy - 1) continue;
      main = Math.max(0, from.top - b.bottom);
      cross = rangeGap(b.left, b.right, from.left, from.right) || Math.abs(cx - fx) * 0.15;
    }
    const score = main + cross * 3;
    if (score < bestScore) {
      bestScore = score;
      best = el;
    }
  }
  return best;
}

/** Пустота между двумя отрезками (0 - пересекаются). */
function rangeGap(a0: number, a1: number, b0: number, b1: number): number {
  if (a1 < b0) return b0 - a1;
  if (b1 < a0) return a0 - b1;
  return 0;
}

export function move(dir: Dir): boolean {
  const list = focusables();
  const cur = current();
  if (!cur) {
    if (list.length) focusEl(list[0]);
    return true;
  }
  const next = pick(dir, cur.getBoundingClientRect(), list, cur);
  if (next) focusEl(next);
  return true;
}

/** Первый элемент с этим ключом (`data-k`) или первый вообще. */
export function focusKey(key: string | null | undefined, root: ParentNode = document) {
  if (key) {
    const all = Array.prototype.slice.call(root.querySelectorAll('.f')) as HTMLElement[];
    for (let i = 0; i < all.length; i++) {
      if (all[i].getAttribute('data-k') === key && isVisible(all[i])) {
        focusEl(all[i]);
        return;
      }
    }
  }
  const list = focusables(root);
  const pref = list.filter((e) => e.hasAttribute('data-first'))[0];
  focusEl(pref || list[0] || null);
}
