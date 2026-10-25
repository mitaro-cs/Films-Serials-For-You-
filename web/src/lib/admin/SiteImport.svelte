<script lang="ts">
  import { post, errorText } from '$lib/api';
  import { session } from '$lib/session.svelte';
  let url=$state(''), busy=$state(false), error=$state('');
  let result=$state<{added:number;existing:number;skipped:{url:string;reason:string}[];note:string}|null>(null);
  async function run(event:SubmitEvent) {
    event.preventDefault(); if(busy)return;
    busy=true;error='';result=null;
    try { result=await post('/api/host/sites/import',{url:url.trim()}); }
    catch(e){error=errorText(e);} finally{busy=false;}
  }
</script>
{#if session.me?.window}
<section class="card pane">
  <h3>Подключить фильмы с сайта</h3>
  <p class="muted">Вставьте адрес страницы с карточками фильмов. Nova добавит до 20 фильмов за раз и пропустит уже подключённые ссылки. Сериалы без списка серий не импортируются.</p>
  <form onsubmit={run}>
    <label class="field"><span>Страница каталога или фильма</span><input class="input" type="url" required bind:value={url} placeholder="https://example.org/films/" disabled={busy}/></label>
    <button class="btn" disabled={busy || !url.trim()}>{busy?'Проверяем страницы…':'Подключить фильмы'}</button>
  </form>
  <div class="presets" aria-label="Сайты для импорта">
    {#each [{name:'Lordfilm',url:'https://gz.lordfilm5.pro/podborki/'},{name:'Gidonline',url:'https://ag.gidonline.fun/home/'},{name:'Kinoklass',url:'https://boevikx1.kinoklass.net/'}] as site}
      <button class="btn" type="button" disabled={busy} onclick={()=>{url=site.url;result=null;error=''}}>{site.name}</button>
    {/each}
  </div>
  <p class="muted">Lordfilm и Gidonline также участвуют в поиске фильма по названию. Kinoklass сейчас отклоняет запросы (HTTP 403). Качество и озвучка зависят от видео на сайте.</p>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if result}<div aria-live="polite"><p>Добавлено: {result.added} · Уже подключено: {result.existing} · Пропущено: {result.skipped.length}</p><p class="muted">{result.note}</p>
    {#if result.skipped.length}<details><summary>Причины пропусков</summary>{#each result.skipped as item}<p class="reason">{item.reason}<br/><span class="muted">{item.url}</span></p>{/each}</details>{/if}
  </div>{/if}
</section>
{/if}
<style>form{display:flex;align-items:end;gap:12px;flex-wrap:wrap}.presets{display:flex;flex-wrap:wrap;gap:8px;margin-top:12px}.field{flex:1;min-width:min(100%,260px)}.reason{overflow-wrap:anywhere;font-size:13px}summary{cursor:pointer}p{line-height:1.55}</style>
