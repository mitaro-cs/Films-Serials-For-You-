// Главная, «Фильмы», «Сериалы»: герой и ряды (те же правила, что у сайта - lib/library.ts).

import type { Card } from '../src/lib/types';
import { featured, ofTab, shelves, type Tab } from '../src/lib/library';
import { bar, library, type Screen } from './app';
import { poster, wide } from './cards';
import { esc, meta } from './util';

function hero(c: Card): string {
  const art = c.backdrop || c.posterLarge || c.poster;
  const play = c.status === 'watching' && c.resume ? 'Продолжить' : 'Смотреть';
  return (
    '<section class="hero"' + (art ? ' style="background-image:url(' + esc(art) + ')"' : '') + '>' +
    '<div class="shade"></div><div class="info">' +
    '<h1>' + esc(c.title) + '</h1>' +
    '<p class="meta">' + esc(meta(c)) + '</p>' +
    (c.overview ? '<p class="ov">' + esc(c.overview) + '</p>' : '') +
    '<div class="btns">' +
    '<a class="f btn pri" data-first data-k="hero-play" data-href="/title/' + c.id + '">' + esc(play) + '</a>' +
    '<a class="f btn" data-k="hero-more" data-href="/title/' + c.id + '">Подробнее</a>' +
    '</div></div></section>'
  );
}

function row(key: string, title: string, items: Card[], wideCards: boolean): string {
  let h = '<section class="sec"><h2>' + esc(title) + '</h2><div class="track">';
  for (let i = 0; i < items.length; i++) h += wideCards ? wide(items[i], key + '-') : poster(items[i], key + '-');
  return h + '</div></section>';
}

export function homeScreen(tab: Tab): Screen {
  return async (root) => {
    const lib = await library(true);
    const items = ofTab(lib.items, tab);
    const active = tab === 'home' ? 'home' : tab;
    if (!items.length) {
      root.innerHTML = bar(active) + '<div class="empty">В этом разделе пока ничего нет</div>';
      return;
    }
    const top = featured(items, 1)[0] || items[0];
    let h = bar(active) + hero(top) + '<div class="rows">';
    const list = shelves(items, lib.recent, tab);
    for (let i = 0; i < list.length; i++) {
      const s = list[i];
      h += row(s.key, s.title, s.items, s.key === 'continue');
    }
    root.innerHTML = h + '</div>';
  };
}
