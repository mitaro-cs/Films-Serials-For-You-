// Поиск по своей библиотеке: набираем на экранной клавиатуре телевизора, список обновляется сразу.

import type { Card } from '../src/lib/types';
import { matches } from '../src/lib/library';
import { bar, library, type Screen } from './app';
import { poster } from './cards';
import { esc } from './util';

function grid(items: Card[]): string {
  if (!items.length) return '<div class="empty small">Ничего не нашлось</div>';
  let h = '';
  for (let i = 0; i < items.length; i++) h += poster(items[i], 'f-');
  return h;
}

export const searchScreen: Screen = async (root, route) => {
  const lib = await library();
  const start = route.query.get('q') || '';
  root.innerHTML =
    bar('search') +
    '<section class="find"><input id="q" class="f field" data-first data-k="q" type="search" placeholder="Название, жанр или режиссёр" autocomplete="off" value="' + esc(start) + '">' +
    '<div class="grid" id="res"></div></section>';
  const input = root.querySelector('#q') as HTMLInputElement;
  const res = root.querySelector('#res') as HTMLElement;
  const show = () => {
    const q = input.value.trim();
    const list = q ? lib.items.filter((c) => matches(c, q)) : lib.items.slice().sort((a, b) => b.addedAt - a.addedAt).slice(0, 24);
    res.innerHTML = grid(list);
  };
  input.addEventListener('input', show);
  show();
};
