// Карточка фильма или сериала: описание, «Смотреть», отметки, серии по сезонам, похожие.

import { api, put } from './api';
import type { Card, FileItem, TitleDetail } from '../src/lib/types';
import { forgetLibrary, bar, type Screen } from './app';
import { poster } from './cards';
import { clock, esc, meta, pct } from './util';
import { focusEl } from './nav';

function episodeLabel(f: FileItem): string {
  if (f.season == null || f.episode == null) return f.name || f.fileName;
  const end = f.episodeEnd && f.episodeEnd !== f.episode ? '-' + f.episodeEnd : '';
  return f.episode + end + ' серия' + (f.name ? '. ' + f.name : '');
}

function episodes(files: FileItem[], season: number): string {
  let h = '';
  for (let i = 0; i < files.length; i++) {
    const f = files[i];
    if ((f.season || 1) !== season) continue;
    const p = f.progress;
    h +=
      '<a class="f card w ep" data-k="ep' + f.id + '" data-href="/watch/' + f.id + '">' +
      '<div class="art">' + (f.still ? '<img src="' + esc(f.still) + '" alt="" draggable="false">' : '<div class="blank"></div>') +
      (p && p.duration ? '<i class="prog"><b style="width:' + (p.done ? 100 : pct(p.position, p.duration)) + '%"></b></i>' : '') +
      '</div><div class="cap"><b>' + esc(episodeLabel(f)) + '</b><small>' +
      esc(p && p.done ? 'Просмотрено' : f.duration ? clock(f.duration) : '') + '</small></div></a>';
  }
  return h;
}

export const titleScreen: Screen = async (root, route) => {
  const id = Number(route.args[0]);
  const d = await api<TitleDetail>('/api/titles/' + id);
  const c: Card = d.card;
  const art = c.backdrop || c.posterLarge || c.poster;
  const series = c.kind === 'series';
  const seasonList = d.seasons.length ? d.seasons : [1];
  let season = d.next ? (d.files.filter((f) => f.id === d.next!.fileId)[0] || ({} as FileItem)).season || seasonList[0] : seasonList[0];
  const target = d.next ? d.next.fileId : d.files.length ? d.files[0].id : 0;
  const playLabel = d.next ? 'Продолжить' + (d.next.label ? ' · ' + d.next.label : '') : series ? 'Смотреть с начала' : 'Смотреть';
  const director = d.people.filter((p) => p.job === 'director' || p.job === 'creator')[0];
  const cast = d.people.filter((p) => p.job === 'actor').slice(0, 5).map((p) => p.name).join(', ');

  const marks = () =>
    '<a class="f btn" data-k="m-want" data-act="want">' + (c.status === 'want' ? '✓ В списке' : '+ Хочу посмотреть') + '</a>' +
    '<a class="f btn" data-k="m-seen" data-act="watched">' + (c.status === 'watched' ? '✓ Просмотрено' : 'Просмотрено') + '</a>';

  let h =
    bar('') +
    '<section class="film" ' + (art ? 'style="background-image:url(' + esc(art) + ')"' : '') + '><div class="shade"></div><div class="info">' +
    '<h1>' + esc(c.title) + '</h1>' +
    (c.originalTitle && c.originalTitle !== c.title ? '<p class="orig">' + esc(c.originalTitle) + '</p>' : '') +
    '<p class="meta">' + esc(meta(c)) + '</p>' +
    (d.tagline ? '<p class="tag">' + esc(d.tagline) + '</p>' : '') +
    (c.overview ? '<p class="ov long">' + esc(c.overview) + '</p>' : '') +
    (director ? '<p class="crew"><span>Режиссёр</span> ' + esc(director.name) + '</p>' : '') +
    (cast ? '<p class="crew"><span>В ролях</span> ' + esc(cast) + '</p>' : '') +
    '<div class="btns">' +
    (target ? '<a class="f btn pri" data-first data-k="play" data-href="/watch/' + target + '">' + esc(playLabel) + '</a>' : '') +
    '<span class="marks">' + marks() + '</span></div></div></section>';

  if (series && d.files.length) {
    h += '<section class="sec"><h2>Серии</h2>';
    if (seasonList.length > 1) {
      h += '<div class="chips" id="seasons">';
      for (let i = 0; i < seasonList.length; i++) {
        h += '<a class="f chip' + (seasonList[i] === season ? ' on' : '') + '" data-k="s' + seasonList[i] + '" data-act="season" data-n="' + seasonList[i] + '">Сезон ' + seasonList[i] + '</a>';
      }
      h += '</div>';
    }
    h += '<div class="track" id="eps">' + episodes(d.files, season) + '</div></section>';
  }
  if (d.similar.length) {
    h += '<section class="sec"><h2>Похожие</h2><div class="track">';
    for (let i = 0; i < d.similar.length; i++) h += poster(d.similar[i], 'sim-');
    h += '</div></section>';
  }
  root.innerHTML = h;

  const onClick = async (e: Event) => {
    const el = (e.target as HTMLElement).closest('[data-act]') as HTMLElement | null;
    if (!el) return;
    const act = el.getAttribute('data-act');
    if (act === 'season') {
      season = Number(el.getAttribute('data-n'));
      const chips = root.querySelectorAll('#seasons .chip');
      for (let i = 0; i < chips.length; i++) chips[i].classList.toggle('on', chips[i] === el);
      const eps = root.querySelector('#eps') as HTMLElement;
      eps.innerHTML = episodes(d.files, season);
      eps.scrollLeft = 0;
      return;
    }
    if (act === 'want' || act === 'watched') {
      const next = c.status === act ? null : act;
      try {
        const r = await put<Card>('/api/titles/' + c.id + '/status', { status: next });
        c.status = r.status;
        forgetLibrary();
        const box = root.querySelector('.marks') as HTMLElement;
        const key = el.getAttribute('data-k');
        box.innerHTML = marks();
        focusEl(box.querySelector('[data-k="' + key + '"]') as HTMLElement);
      } catch (err) {
        /* не вышло - оставляем как было */
      }
    }
  };
  root.addEventListener('click', onClick);
  return () => root.removeEventListener('click', onClick);
};
