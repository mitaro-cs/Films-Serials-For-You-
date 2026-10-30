<script lang="ts">
  import { api, put } from '$lib/api';
  import { session } from '$lib/session.svelte';
  import type { ExtCard } from '$lib/types';
  import Icon from './Icon.svelte';

  const choices = ['Боевик','Приключения','Комедия','Драма','Фантастика','Фэнтези','Ужасы','Триллер','Детектив','Анимация'];
  let open = $state(false), step = $state(0), busy = $state(false), error = $state('');
  let genres = $state<string[]>([]), films = $state<ExtCard[]>([]), selected = $state<string[]>([]);
  const profile = $derived(session.me?.user.profile?.id);
  $effect(() => {
    if (!profile || sessionStorage.getItem(`nova-onboarding:${profile}`) !== '1') return;
    open = true;
    api<{rows:{items:ExtCard[]}[]}>('/api/discover').then(r => {
      films = [...new Map(r.rows.flatMap(row => row.items).filter(f => f.kind === 'movie').map(f => [f.id,f])).values()].slice(0,18);
    }).catch(() => {});
  });
  function toggle<T>(items:T[], item:T):T[] { return items.includes(item) ? items.filter(x => x !== item) : [...items,item]; }
  async function finish() {
    if (!profile || busy) return;
    busy = true; error = '';
    try {
      for (const id of selected) {
        const result = await put<{favorites:string[]}>('/api/me/favorites',{key:`x:${id}`,value:true});
        if (session.me) session.me.favorites = result.favorites;
      }
      localStorage.setItem(`nova-genres:${profile}`,JSON.stringify(genres));
      sessionStorage.removeItem(`nova-onboarding:${profile}`);
      window.dispatchEvent(new Event('nova-preferences'));
      open = false;
    } catch { error = 'Не удалось сохранить выбор. Проверьте соединение и попробуйте снова.'; }
    finally { busy = false; }
  }
  function skip() { if(profile) sessionStorage.removeItem(`nova-onboarding:${profile}`); open=false; }
</script>

{#if open}
  <div class="veil"><div class="guide" role="dialog" aria-modal="true" aria-label="Знакомство с Nova" tabindex="-1">
    <div class="top"><span>Знакомство с Nova · {step+1}/3</span><button class="skip" onclick={skip}>Пропустить</button></div>
    {#if step === 0}
      <div class="symbol"><Icon name="play" size={32} /></div><h1>Истории становятся ближе</h1>
      <p>Ищите фильмы, сохраняйте любимое и смотрите вместе с друзьями. Мы поможем найти то, что понравится именно вам.</p>
    {:else if step === 1}
      <h1>Какие жанры вам нравятся?</h1><p>Выберите несколько — мы покажем подходящие фильмы на главной.</p>
      <div class="choices">{#each choices as genre}<button class:on={genres.includes(genre)} aria-pressed={genres.includes(genre)} onclick={() => genres=toggle(genres,genre)}>{genre}</button>{/each}</div>
    {:else}
      <h1>Выберите любимые фильмы</h1><p>Их можно будет найти в профиле. Выбор необязателен.</p>
      {#if films.length}<div class="films">{#each films as film}<button class:on={selected.includes(film.id)} aria-pressed={selected.includes(film.id)} onclick={() => selected=toggle(selected,film.id)}>{#if film.poster}<img src={film.poster} alt="" />{/if}<span>{film.title}</span></button>{/each}</div>{:else}<p>Подборка пока недоступна — любимые фильмы можно выбрать позже.</p>{/if}
    {/if}
    {#if error}<p role="alert">{error}</p>{/if}
    <div class="actions">{#if step > 0}<button class="btn" onclick={() => step--}>Назад</button>{/if}<button class="btn primary" disabled={busy} onclick={() => step < 2 ? step++ : finish()}>{step < 2 ? 'Продолжить' : busy ? 'Сохраняем…' : 'Начать смотреть'}</button></div>
  </div></div>
{/if}
<style>
  .veil{position:fixed;inset:0;z-index:2200;display:grid;place-items:center;padding:20px;background:#000c;animation:fade .25s ease}
  .guide{width:min(680px,100%);max-height:90dvh;overflow:auto;padding:clamp(22px,5vw,40px);border:1px solid #ffffff24;border-radius:24px;background:#1b1b1d;animation:rise .35s ease;color:#fff}
  .top{display:flex;justify-content:space-between;color:#aaa;font-size:13px}.skip{border:0;background:none;color:#aaa;cursor:pointer}
  h1{font-size:clamp(26px,5vw,38px);margin:22px 0 10px}p{color:#bbb;line-height:1.5}.symbol{display:grid;place-items:center;width:64px;height:64px;border-radius:50%;background:#fff;color:#111;margin-top:24px}
  .choices{display:flex;flex-wrap:wrap;gap:8px;margin:24px 0}.choices button{padding:10px 16px;border:1px solid #555;border-radius:999px;background:#292929;color:#fff;cursor:pointer}.choices button.on,.films button.on{outline:2px solid white;background:#444}
  .films{display:grid;grid-template-columns:repeat(auto-fill,minmax(95px,1fr));gap:10px;max-height:38dvh;overflow:auto}.films button{background:#292929;border:0;border-radius:8px;color:#fff;overflow:hidden;padding:0 0 7px;cursor:pointer}.films img{width:100%;aspect-ratio:2/3;object-fit:cover}.films span{display:block;padding:5px;font-size:12px}.actions{display:flex;justify-content:flex-end;gap:10px;margin-top:28px}
  @keyframes fade{from{opacity:0}}@keyframes rise{from{opacity:0;transform:translateY(18px)}}
</style>
