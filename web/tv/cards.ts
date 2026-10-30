// Карточки: постер 2:3 и широкая (кадр 16:9 с полосой «где остановился»).

import type { Card } from '../src/lib/types';
import { caption } from '../src/lib/library';
import { esc, pct } from './util';

function img(src: string | null): string {
  return src ? '<img src="' + esc(src) + '" alt="" draggable="false">' : '<div class="blank"></div>';
}

export function poster(c: Card, key: string): string {
  return (
    '<a class="f card p" data-k="' + esc(key + c.id) + '" data-href="/title/' + c.id + '">' +
    '<div class="art">' + img(c.poster) +
    (c.status === 'watched' ? '<span class="mark">Просмотрено</span>' : '') + '</div>' +
    '<div class="cap"><b>' + esc(c.title) + '</b>' + (c.year ? '<small>' + c.year + '</small>' : '') + '</div></a>'
  );
}

export function wide(c: Card, key: string): string {
  const r = c.resume;
  const bar = r && r.duration ? '<i class="prog"><b style="width:' + pct(r.position, r.duration) + '%"></b></i>' : '';
  return (
    '<a class="f card w" data-k="' + esc(key + c.id) + '" data-href="/title/' + c.id + '">' +
    '<div class="art">' + img(c.backdropSmall || c.backdrop || c.poster) + bar + '</div>' +
    '<div class="cap"><b>' + esc(c.title) + '</b><small>' + esc(caption(c)) + '</small></div></a>'
  );
}
