<script lang="ts">
  import { api } from '$lib/api';
  import { clock, size } from '$lib/fmt';
  import { percent } from '$lib/library';
  import Icon from '$lib/ui/Icon.svelte';
  import type { IconName } from '$lib/ui/icons';
  import type { Status } from './types';

  // Обзор для хоста: сводка «всё ли в порядке», чек-лист с пояснениями и ссылками, куда идти,
  // цифры с объяснением и кто сейчас смотрит. Показатели обновляются раз в 5 секунд без
  // перестройки страницы: меняются только числа (размеры плиток и строк заданы заранее).
  let { status, reload }: { status: Status; reload: () => void } = $props();
  let copied = $state(false);
  let people = $state<number | null>(null);

  $effect(() => {
    const t = setInterval(() => document.visibilityState === 'visible' && reload(), 5000);
    return () => clearInterval(t);
  });
  $effect(() => {
    api<unknown[]>('/api/admin/users').then((u) => (people = u.length), () => {});
  });

  async function copy() {
    if (!status.publicUrl) return;
    await navigator.clipboard?.writeText(status.publicUrl).catch(() => {});
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }

  type Check = { ok: boolean | null; icon: IconName; title: string; text: string; href?: string; action?: string };
  const GB = 1024 ** 3;
  const diskLow = $derived(status.system?.diskFree != null && status.system.diskFree < 10 * GB);
  const checks = $derived<Check[]>([
    {
      ok: true,
      icon: 'pulse',
      title: 'Сервер работает',
      text: `Nova ${status.version} раздаёт фильмы с этого компьютера. Пока он включён, зрители могут смотреть.`,
    },
    {
      ok: !!status.publicUrl,
      icon: 'globe',
      title: status.publicUrl ? 'Доступ из интернета включён' : 'Доступ из интернета выключен',
      text: status.publicUrl
        ? 'Друзья заходят по постоянному адресу ниже - его можно отправить один раз.'
        : 'Сейчас Nova видно только в вашей домашней сети. Включите доступ, чтобы смотрели друзья из других мест.',
      href: '?tab=access',
      action: status.publicUrl ? 'Настроить' : 'Включить',
    },
    {
      ok: status.ffmpeg && status.ffprobe,
      icon: 'film',
      title: status.ffmpeg && status.ffprobe ? 'ffmpeg на месте' : 'Нет ffmpeg',
      text:
        status.ffmpeg && status.ffprobe
          ? 'MKV, AVI, несколько озвучек и субтитры играют в любом браузере, качество подстраивается под интернет.'
          : 'Без ffmpeg не играют MKV/AVI и фильмы с несколькими озвучками. Укажите путь в «Настройках».',
      href: status.ffmpeg && status.ffprobe ? undefined : '?tab=settings',
      action: 'Открыть настройки',
    },
    {
      ok: status.scan.error ? false : status.titles > 0 ? true : null,
      icon: 'folder',
      title: status.scan.error ? 'Ошибка в папках' : status.titles ? 'Папки с фильмами подключены' : 'Фильмов пока нет',
      text: status.scan.error
        ? status.scan.error
        : status.titles
          ? `${status.titles} фильмов и сериалов, ${status.files} файлов (${size(status.size)}). Новое в папках появляется само.`
          : 'Добавьте папку с фильмами или перетащите файлы в окно Nova - они появятся на сайте сами.',
      href: '?tab=folders',
      action: status.titles ? 'Папки' : 'Добавить папку',
    },
    {
      ok: status.meta.error ? false : status.metaPending ? null : true,
      icon: 'image',
      title: status.meta.error ? 'Описания не загружаются' : status.metaPending ? 'Загружаем описания' : 'Описания загружены',
      text: status.meta.error
        ? status.meta.error
        : status.metaPending
          ? `Осталось ${status.metaPending}: постеры, кадры, актёры и сюжет по-русски подтягиваются в фоне.`
          : 'У всех фильмов есть постер, описание и оценки.',
      href: '?tab=catalog',
      action: 'Каталог',
    },
    {
      ok: status.system?.supported ? !diskLow : null,
      icon: 'download',
      title: diskLow ? 'Мало места на диске' : 'Место на диске',
      text:
        status.system?.diskFree != null
          ? `Свободно ${size(status.system.diskFree)} из ${size(status.system.diskTotal ?? 0)}.${diskLow ? ' Нужно хотя бы 10 ГБ - на копии и кэш видео.' : ''}`
          : 'Свободное место покажется после первого замера.',
    },
    {
      ok: status.update?.state === 'available' ? null : true,
      icon: 'refresh',
      title: status.update?.state === 'available' ? `Есть новая версия ${status.update.latest ?? ''}` : 'Версия',
      text: status.update?.state === 'available' ? 'Установщик обновит Nova, фильмы и люди сохранятся.' : `Установлена ${status.version}.`,
      href: '?tab=version',
      action: 'Подробнее',
    },
  ]);
  const problems = $derived(checks.filter((c) => c.ok === false).length);
  const waiting = $derived(checks.filter((c) => c.ok === null).length);
</script>

<section class="summary" class:bad={problems > 0} aria-label="Сводка">
  <span class="dot" aria-hidden="true"></span>
  <div>
    <h2>{problems ? `Нужно внимание: ${problems}` : waiting ? 'Всё работает, кое-что ещё в процессе' : 'Всё в порядке'}</h2>
    <p class="muted">
      {problems
        ? 'Ниже отмечено, что не так и куда нажать, чтобы исправить.'
        : 'Nova раздаёт фильмы, зрители могут смотреть. Ниже - подробности по каждой части.'}
    </p>
  </div>
</section>

<div class="tiles">
  <div class="tile">
    <span>Фильмы и сериалы</span>
    <b>{status.titles}</b>
    <small>на сайте сейчас</small>
  </div>
  <div class="tile">
    <span>Файлы</span>
    <b>{status.files}</b>
    <small>{size(status.size)} на дисках</small>
  </div>
  <div class="tile">
    <span>Зрители</span>
    <b>{people ?? '…'}</b>
    <small>аккаунтов, вместе с вами</small>
  </div>
  <div class="tile">
    <span>Смотрят сейчас</span>
    <b>{status.watching.length}</b>
    <small>{status.watching.length ? 'человек у экрана' : 'никого'}</small>
  </div>
</div>

<section class="card pane" aria-label="Состояние">
  <h3><Icon name="check" size={18} /> Состояние</h3>
  <ul class="checks">
    {#each checks as c (c.title + c.icon)}
      <li class:ok={c.ok === true} class:bad={c.ok === false} class:wait={c.ok === null}>
        <span class="ic"><Icon name={c.icon} size={18} /></span>
        <div>
          <b>{c.title}</b>
          <p>{c.text}</p>
        </div>
        {#if c.href}<a class="btn sm" href={c.href} data-sveltekit-replacestate data-sveltekit-noscroll>{c.action}</a>{/if}
      </li>
    {/each}
  </ul>
</section>

<section class="card pane" aria-label="Адрес для зрителей">
  <h3><Icon name="link" size={18} /> Адрес для зрителей</h3>
  {#if status.publicUrl}
    <p class="muted small">Отправьте этот адрес друзьям. Он не меняется; работает, пока компьютер включён.</p>
    <div class="url">
      <code>{status.publicUrl}</code>
      <button class="btn sm" onclick={copy}><Icon name={copied ? 'check' : 'copy'} size={15} /> {copied ? 'Скопировано' : 'Копировать'}</button>
    </div>
  {:else}
    <p class="muted">Доступ из интернета выключен. Вкладка «Доступ» - включить в пару нажатий.</p>
  {/if}
</section>

{#if status.system?.supported}
  <section class="card pane" aria-label="Компьютер хоста">
    <h3><Icon name="pulse" size={18} /> Компьютер хоста</h3>
    <p class="muted small">Насколько занят компьютер, который раздаёт фильмы. Высокая загрузка процессора - обычно перекодирование видео для слабого интернета у зрителя.</p>
    <div class="meters">
      <div class="meter">
        <span>Процессор</span>
        <b>{status.system.cpu == null ? '…' : `${Math.round(status.system.cpu)}%`}</b>
        <i style:transform="scaleX({(status.system.cpu ?? 0) / 100})"></i>
      </div>
      <div class="meter">
        <span>Память</span>
        <b>{status.system.memoryUsed == null ? '-' : size(status.system.memoryUsed)}</b>
        <i style:transform="scaleX({status.system.memoryUsed && status.system.memoryTotal ? status.system.memoryUsed / status.system.memoryTotal : 0})"></i>
      </div>
      <div class="meter" class:low={diskLow}>
        <span>Диск занят</span>
        <b>{status.system.diskFree == null || !status.system.diskTotal ? '-' : `${Math.round(100 - (status.system.diskFree / status.system.diskTotal) * 100)}%`}</b>
        <i style:transform="scaleX({status.system.diskFree != null && status.system.diskTotal ? 1 - status.system.diskFree / status.system.diskTotal : 0})"></i>
      </div>
    </div>
    <p class="faint small">Обновляется каждые 5 секунд, пока вкладка открыта.</p>
  </section>
{/if}

<section class="card pane" aria-label="Сейчас смотрят">
  <h3><Icon name="play" size={18} /> Сейчас смотрят</h3>
  {#if status.watching.length}
    <ul class="watching">
      {#each status.watching as w, i (i)}
        <li>
          <b>{w.user}</b>
          <span class="what">{w.title}{w.episode ? ` · ${w.episode}` : ''}</span>
          <span class="time">{clock(w.position)} / {clock(w.duration)}</span>
          <span class="bar"><i style:transform="scaleX({percent(w.position, w.duration) / 100})"></i></span>
          {#if w.net}
            <span class="net" class:slow={w.net.stalls > 0}>
              {w.net.mbit !== null ? `${w.net.mbit} Мбит/с` : 'скорость не замерена'} · {w.net.quality === 'orig' ? 'оригинал' : w.net.quality === 'auto' ? 'автокачество' : w.net.quality}
              {#if w.net.stalls > 0} · остановок: {w.net.stalls}{/if}
            </span>
          {/if}
        </li>
      {/each}
    </ul>
  {:else}
    <p class="muted">Никто не смотрит. Когда кто-то включит фильм, он появится здесь с местом просмотра, скоростью интернета и качеством.</p>
  {/if}
</section>

<style>
  .summary {
    display: flex;
    align-items: center;
    gap: 16px;
    margin-bottom: 18px;
    padding: 18px 20px;
    border-radius: 20px;
    background: rgba(48, 209, 88, 0.08);
    border: 1px solid rgba(48, 209, 88, 0.3);
  }
  .summary.bad {
    background: rgba(255, 92, 108, 0.08);
    border-color: rgba(255, 92, 108, 0.35);
  }
  .summary h2 {
    margin: 0 0 2px;
    font-size: 20px;
  }
  .summary p {
    margin: 0;
  }
  .dot {
    flex: none;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: #30d158;
    box-shadow: 0 0 0 6px rgba(48, 209, 88, 0.18);
  }
  .summary.bad .dot {
    background: var(--danger);
    box-shadow: 0 0 0 6px rgba(255, 92, 108, 0.18);
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 170px), 1fr));
    gap: 12px;
    margin-bottom: 18px;
  }
  .tile {
    display: grid;
    gap: 2px;
    min-height: 104px;
    padding: 16px 18px;
    border-radius: 18px;
    background: #0b0b0d;
    border: 1px solid var(--line);
  }
  .tile span {
    font-size: 13px;
    color: var(--muted);
  }
  .tile b {
    font-size: 30px;
    line-height: 1.15;
    font-variant-numeric: tabular-nums;
  }
  .tile small {
    font-size: 12px;
    color: var(--faint);
  }
  .pane {
    margin-bottom: 16px;
    padding: 18px 20px;
  }
  .pane h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 12px;
    font-size: 16px;
  }
  .small {
    font-size: 13px;
    margin: 0 0 12px;
  }
  .checks {
    display: grid;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .checks li {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 14px;
    padding: 12px 4px;
    border-top: 1px solid var(--line);
  }
  .checks li:first-child {
    border-top: 0;
  }
  .checks .ic {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.06);
  }
  .checks .ok .ic {
    background: rgba(48, 209, 88, 0.14);
    color: #30d158;
  }
  .checks .bad .ic {
    background: rgba(255, 92, 108, 0.14);
    color: var(--danger);
  }
  .checks .wait .ic {
    background: rgba(255, 184, 77, 0.14);
    color: var(--warn);
  }
  .checks b {
    display: block;
    font-size: 14px;
  }
  .checks p {
    margin: 2px 0 0;
    font-size: 13px;
    color: var(--muted);
    overflow-wrap: anywhere;
  }
  .url {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
  }
  .url code {
    min-width: 0;
    padding: 8px 12px;
    border-radius: 10px;
    background: #000;
    overflow-wrap: anywhere;
  }
  .meters {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 180px), 1fr));
    gap: 12px;
    margin-bottom: 10px;
  }
  .meter {
    position: relative;
    display: grid;
    gap: 2px;
    padding: 14px 16px 18px;
    overflow: hidden;
    border-radius: 14px;
    background: #0b0b0d;
    border: 1px solid var(--line);
  }
  .meter span {
    font-size: 12px;
    color: var(--muted);
  }
  .meter b {
    font-size: 22px;
    font-variant-numeric: tabular-nums;
  }
  .meter i {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 4px;
    background: #fff;
    transform-origin: left;
    transition: transform 0.6s ease;
  }
  .meter.low i {
    background: var(--danger);
  }
  .watching {
    display: grid;
    gap: 10px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .watching li {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: baseline;
    gap: 4px 12px;
  }
  .watching .what {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    color: var(--muted);
  }
  .watching .time {
    font-size: 12px;
    color: var(--faint);
    font-variant-numeric: tabular-nums;
  }
  /* Скорость и качество у зрителя: остановки - заметнее. */
  .watching .net {
    grid-column: 1 / -1;
    font-size: 12px;
    color: var(--faint);
    font-variant-numeric: tabular-nums;
  }
  .watching .net.slow {
    color: #f5c46b;
  }
  .watching .bar {
    grid-column: 1 / -1;
    height: 3px;
    overflow: hidden;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.12);
  }
  .watching .bar i {
    display: block;
    height: 100%;
    background: #fff;
    transform-origin: left;
    transition: transform 0.6s ease;
  }
  @media (max-width: 640px) {
    .checks li {
      grid-template-columns: auto minmax(0, 1fr);
    }
    .checks li .btn {
      grid-column: 2;
      justify-self: start;
    }
  }
</style>
