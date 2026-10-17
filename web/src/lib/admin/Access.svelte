<script lang="ts">
  import { api, errorText, post, put } from '$lib/api';
  import Icon from '$lib/ui/Icon.svelte';

  interface View {
    mode: 'lan' | 'cloudpub' | 'fxtunnel' | 'custom';
    state: string;
    message: string | null;
    publicUrl: string | null;
    lanUrl: string | null;
    subdomain: string;
    fxLoggedIn: boolean;
    cpLoggedIn: boolean;
    login: { code: string; url: string; expiresAt: number } | null;
    customUrl: string;
  }

  let v = $state<View | null>(null);
  let sub = $state('');
  let custom = $state('');
  let error = $state('');
  let busy = $state(false);
  let copied = $state('');
  // Вход в CloudPub: почта и пароль или ключ API (если на cloudpub.ru входят через Яндекс/VK).
  let cpEmail = $state('');
  let cpPass = $state('');
  let cpToken = $state('');
  let cpByToken = $state(false);

  function load() {
    api<View>('/api/admin/access').then((x) => {
      const first = !v;
      v = x;
      if (first) {
        sub = x.subdomain;
        custom = x.customUrl;
      }
    }, (e) => (error = errorText(e)));
  }
  $effect(() => {
    load();
    const t = setInterval(load, 2500);
    return () => clearInterval(t);
  });

  async function set(mode: View['mode']) {
    error = '';
    busy = true;
    try {
      v = await put<View>('/api/admin/access', { mode, subdomain: mode === 'fxtunnel' && sub ? sub : undefined, url: mode === 'custom' ? custom : undefined });
    } catch (x) {
      error = errorText(x);
    } finally {
      busy = false;
    }
  }

  async function login() {
    error = '';
    try {
      const l = await post<{ url: string }>('/api/admin/access/login');
      window.open(l.url, '_blank', 'noopener');
      load();
    } catch (x) {
      error = errorText(x);
    }
  }

  async function cpLogin(e: SubmitEvent) {
    e.preventDefault();
    error = '';
    busy = true;
    try {
      v = await post<View>('/api/admin/access/cloudpub/login', cpByToken ? { token: cpToken } : { email: cpEmail, password: cpPass });
      cpPass = '';
      cpToken = '';
    } catch (x) {
      error = errorText(x);
    } finally {
      busy = false;
    }
  }

  async function cpLogout() {
    v = await post<View>('/api/admin/access/cloudpub/logout').catch(() => v);
  }

  async function copy(t: string) {
    await navigator.clipboard?.writeText(t).catch(() => {});
    copied = t;
    setTimeout(() => (copied = ''), 1500);
  }

  const STATE: Record<string, string> = {
    online: 'Работает',
    starting: 'Подключаемся…',
    retrying: 'Переподключаемся…',
    needs_login: 'Нужен вход',
    error: 'Ошибка',
    off: 'Выключено',
  };
</script>

{#if v}
  {#if error}<p class="error">{error}</p>{/if}
  <div class="modes">
    <button class="mode card" class:on={v.mode === 'lan'} onclick={() => set('lan')} disabled={busy}>
      <Icon name="home" size={22} />
      <b>Только дома</b>
      <span class="muted">Смотрят те, кто подключён к вашему Wi-Fi</span>
    </button>
    <button class="mode card" class:on={v.mode === 'cloudpub'} onclick={() => set('cloudpub')} disabled={busy}>
      <Icon name="globe" size={22} />
      <b>Ссылка для всех <small class="rec">рекомендуем</small></b>
      <span class="muted">CloudPub: бесплатная постоянная ссылка https://….cloudpub.ru — открывается в России с VPN и без</span>
    </button>
    <button class="mode card" class:on={v.mode === 'fxtunnel'} onclick={() => set('fxtunnel')} disabled={busy}>
      <Icon name="globe" size={22} />
      <b>fxTunnel</b>
      <span class="muted">Запасной бесплатный туннель: свой адрес https://….fxtun.ru</span>
    </button>
    <button class="mode card" class:on={v.mode === 'custom'} onclick={() => set('custom')} disabled={busy || !custom}>
      <Icon name="link" size={22} />
      <b>Свой адрес</b>
      <span class="muted">Если на роутере проброшен порт или есть свой домен</span>
    </button>
  </div>

  {#if v.lanUrl}
    <div class="card pane">
      <h3>В домашней сети</h3>
      <div class="url"><code>{v.lanUrl}</code><button class="btn sm" onclick={() => copy(v!.lanUrl!)}><Icon name={copied === v.lanUrl ? 'check' : 'copy'} size={14} /> Копировать</button></div>
    </div>
  {/if}

  {#if v.mode === 'cloudpub'}
    <div class="card pane">
      <h3>CloudPub <span class="st {v.state}">{STATE[v.state] ?? v.state}</span></h3>
      {#if v.message}<p class="muted">{v.message}</p>{/if}
      {#if v.publicUrl}
        <div class="url"><code>{v.publicUrl}</code><button class="btn sm primary" onclick={() => copy(v!.publicUrl!)}><Icon name={copied === v.publicUrl ? 'check' : 'copy'} size={14} /> Копировать</button></div>
        <p class="muted small">Эта ссылка не меняется — её можно один раз отправить друзьям. Работает, пока компьютер включён.</p>
      {/if}
      {#if !v.cpLoggedIn}
        <p>Один раз войдите в бесплатный аккаунт <a href="https://cloudpub.ru" target="_blank" rel="noopener noreferrer">cloudpub.ru</a> — дальше ссылка поднимается сама при каждом запуске.</p>
        <form class="cp" onsubmit={cpLogin}>
          {#if cpByToken}
            <label class="field"><span>Ключ API из личного кабинета</span><input class="input" bind:value={cpToken} autocomplete="off" required /></label>
          {:else}
            <label class="field"><span>Почта</span><input class="input" type="email" bind:value={cpEmail} autocomplete="username" required /></label>
            <label class="field"><span>Пароль</span><input class="input" type="password" bind:value={cpPass} autocomplete="current-password" required /></label>
          {/if}
          <div class="row">
            <button class="btn primary" disabled={busy}><Icon name="user" size={15} /> {busy ? 'Входим…' : 'Войти в CloudPub'}</button>
            <button type="button" class="btn ghost sm" onclick={() => (cpByToken = !cpByToken)}>{cpByToken ? 'Войти почтой' : 'Вход через Яндекс или VK? Ключом API'}</button>
          </div>
          <p class="muted small">Пароль не сохраняется — CloudPub выдаёт приложению свой ключ.</p>
        </form>
      {:else}
        <div class="row"><button class="btn sm ghost" onclick={cpLogout}>Выйти из CloudPub</button></div>
      {/if}
    </div>
  {/if}

  {#if v.mode === 'fxtunnel'}
    <div class="card pane">
      <h3>fxTunnel <span class="st {v.state}">{STATE[v.state] ?? v.state}</span></h3>
      {#if v.message}<p class="muted">{v.message}</p>{/if}
      {#if v.publicUrl}
        <div class="url"><code>{v.publicUrl}</code><button class="btn sm primary" onclick={() => copy(v!.publicUrl!)}><Icon name={copied === v.publicUrl ? 'check' : 'copy'} size={14} /> Копировать</button></div>
      {/if}
      {#if v.login}
        <div class="code">
          <p>Подтвердите вход на сайте fxTunnel, код:</p>
          <b>{v.login.code}</b>
          <a class="btn" href={v.login.url} target="_blank" rel="noopener noreferrer"><Icon name="globe" size={15} /> Открыть fxTunnel</a>
        </div>
      {:else if !v.fxLoggedIn}
        <p>Один раз войдите в fxTunnel (бесплатно, через браузер) — дальше всё подключается само.</p>
        <button class="btn primary" onclick={login}><Icon name="user" size={15} /> Войти в fxTunnel</button>
      {/if}
      {#if v.fxLoggedIn}
        <form class="sub" onsubmit={(e) => (e.preventDefault(), set('fxtunnel'))}>
          <label class="field">
            <span>Адрес сайта</span>
            <div class="subline"><span class="muted">https://</span><input class="input" bind:value={sub} /><span class="muted">.fxtun.ru</span></div>
          </label>
          <button class="btn sm">Сохранить</button>
          <button type="button" class="btn sm ghost" onclick={() => post('/api/admin/access/logout').then(load)}>Выйти из fxTunnel</button>
        </form>
      {/if}
    </div>
  {/if}

  <div class="card pane">
    <h3>Свой адрес</h3>
    <form class="sub" onsubmit={(e) => (e.preventDefault(), set('custom'))}>
      <input class="input" bind:value={custom} placeholder="https://kino.example.ru" />
      <button class="btn sm" disabled={!custom}>Использовать</button>
    </form>
    <p class="muted small">Порт для проброса на роутере — тот, что в адресе домашней сети.</p>
  </div>
{/if}

<style>
  .modes {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(220px, 100%), 1fr));
    gap: 12px;
    margin-bottom: 16px;
  }
  .mode {
    display: grid;
    gap: 6px;
    justify-items: start;
    padding: 18px;
    text-align: left;
    cursor: pointer;
    transition: border-color 0.15s;
  }
  .mode span {
    font-size: 13px;
  }
  .mode.on {
    border-color: #fff;
    background: rgba(255, 255, 255, 0.1);
  }
  .pane {
    padding: 18px 20px;
    margin-bottom: 14px;
    display: grid;
    gap: 12px;
  }
  h3 {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 16px;
  }
  .st {
    font-size: 12px;
    padding: 2px 8px;
    border-radius: 99px;
    background: var(--surface-2);
    font-weight: 600;
  }
  .st.online {
    background: rgba(61, 220, 151, 0.18);
    color: var(--ok);
  }
  .st.retrying,
  .st.needs_login,
  .st.error {
    color: var(--warn);
  }
  p {
    margin: 0;
  }
  .url {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
  }
  code {
    padding: 6px 10px;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.35);
    font-size: 14px;
  }
  .code {
    display: grid;
    gap: 10px;
    justify-items: start;
  }
  .code b {
    font-size: 28px;
    letter-spacing: 0.12em;
  }
  .sub {
    display: flex;
    flex-wrap: wrap;
    align-items: end;
    gap: 10px;
  }
  .sub .input {
    flex: 1;
    min-width: min(200px, 100%);
  }
  .subline {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .small {
    font-size: 13px;
  }
  .rec {
    margin-left: 4px;
    padding: 1px 7px;
    border-radius: 99px;
    background: #fff;
    color: var(--on-accent);
    font-size: 10.5px;
    font-weight: 700;
    vertical-align: 2px;
  }
  .cp {
    display: grid;
    gap: 12px;
    max-width: 420px;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
  }
  .url code {
    overflow-wrap: anywhere;
    min-width: 0;
  }
</style>
