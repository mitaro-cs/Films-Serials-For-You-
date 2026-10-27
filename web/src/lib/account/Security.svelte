<script lang="ts">
  import { api, errorText, post } from '$lib/api';
  import { toast } from '$lib/ui/toast.svelte';

  // Вход в два шага (код из приложения-аутентификатора) и последние входы в аккаунт.
  type Login = { ok: boolean; agent: string | null; at: number };
  let info = $state<{ totp: boolean; logins: Login[] } | null>(null);
  let setup = $state<{ secret: string; uri: string } | null>(null);
  let code = $state('');
  let busy = $state(false);
  let error = $state('');

  function load() {
    api<{ totp: boolean; logins: Login[] }>('/api/me/security').then((r) => (info = r), (e) => (error = errorText(e)));
  }
  $effect(load);

  async function start() {
    error = '';
    code = '';
    try {
      setup = await post('/api/me/totp/setup');
    } catch (e) {
      error = errorText(e);
    }
  }

  async function confirm(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    error = '';
    try {
      if (info?.totp) {
        await api('/api/me/totp', { method: 'DELETE', json: { code } });
        toast('Вход в два шага выключен');
      } else {
        await api('/api/me/totp', { method: 'PUT', json: { code } });
        toast('Вход в два шага включён');
      }
      setup = null;
      code = '';
      load();
    } catch (x) {
      error = errorText(x);
    } finally {
      busy = false;
    }
  }

  // Браузер и система одной строкой: «Chrome, Windows».
  function device(agent: string | null): string {
    if (!agent) return 'Неизвестное устройство';
    const browser = /Edg\//.test(agent) ? 'Edge' : /YaBrowser/.test(agent) ? 'Яндекс Браузер' : /Firefox\//.test(agent) ? 'Firefox' : /Chrome\//.test(agent) ? 'Chrome' : /Safari\//.test(agent) ? 'Safari' : 'Браузер';
    const os = /iPhone|iPad/.test(agent) ? 'iPhone/iPad' : /Android/.test(agent) ? 'Android' : /Windows/.test(agent) ? 'Windows' : /Mac OS/.test(agent) ? 'Mac' : /Linux/.test(agent) ? 'Linux' : '';
    return os ? `${browser}, ${os}` : browser;
  }
  const when = (ms: number) => new Date(ms).toLocaleString('ru-RU', { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' });
  const grouped = (s: string) => s.match(/.{1,4}/g)?.join(' ') ?? s;
</script>

<section class="card pane" aria-labelledby="security-title">
  <h2 id="security-title">Безопасность</h2>
  {#if info}
    <div class="row">
      <div>
        <b>Вход в два шага</b>
        <p class="muted small">
          {info.totp ? 'Включён: после пароля Nova спросит код из приложения на телефоне.' : 'После пароля - ещё код из приложения на телефоне. Украденный пароль без телефона не поможет.'}
        </p>
      </div>
      <span class="state" class:on={info.totp}>{info.totp ? 'Включён' : 'Выключен'}</span>
    </div>

    {#if setup}
      <ol class="steps">
        <li>Поставьте на телефон Google Authenticator или Яндекс Ключ.</li>
        <li>
          Добавьте аккаунт: <a class="link" href={setup.uri}>открыть в приложении</a> (с телефона) или введите ключ вручную:
          <code class="selectable">{grouped(setup.secret)}</code>
        </li>
        <li>Введите 6 цифр, которые покажет приложение.</li>
      </ol>
    {/if}

    {#if setup || info.totp}
      <form class="confirm" onsubmit={confirm}>
        <input class="input code" bind:value={code} inputmode="numeric" autocomplete="one-time-code" maxlength="7" placeholder="000 000" aria-label="Код из приложения" required />
        <button class="btn" class:danger={info.totp} disabled={busy}>{info.totp ? 'Выключить' : 'Включить'}</button>
      </form>
    {:else}
      <button class="btn" onclick={start}>Включить вход в два шага</button>
    {/if}

    {#if info.logins.length}
      <h3>Последние входы по паролю</h3>
      <ul class="log">
        {#each info.logins as l (l.at)}
          <li class:bad={!l.ok}><span>{device(l.agent)}</span><small>{l.ok ? '' : 'неверный пароль или код - '}{when(l.at)}</small></li>
        {/each}
      </ul>
      <p class="muted small">Видите вход, который были не вы, - смените пароль: остальные устройства выйдут.</p>
    {/if}
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</section>

<style>
  .pane {
    display: grid;
    gap: 14px;
    padding: 22px;
    margin-bottom: 18px;
  }
  h2 {
    font-size: 17px;
  }
  h3 {
    margin-top: 6px;
    font-size: 14px;
    color: var(--muted);
  }
  .row {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }
  .row p {
    margin: 4px 0 0;
  }
  .state {
    flex: none;
    padding: 3px 10px;
    border-radius: 99px;
    background: var(--surface-2);
    font-size: 12px;
    font-weight: 700;
  }
  .state.on {
    background: #1d3b2a;
    color: #9ce5b8;
  }
  .steps {
    margin: 0;
    padding-left: 20px;
    display: grid;
    gap: 8px;
    font-size: 14px;
  }
  code {
    display: block;
    margin-top: 6px;
    padding: 8px 10px;
    border-radius: 8px;
    background: var(--surface-2);
    font-size: 14px;
    letter-spacing: 0.08em;
    overflow-wrap: anywhere;
  }
  .link {
    text-decoration: underline;
  }
  .confirm {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .code {
    flex: 1 1 140px;
    max-width: 200px;
    font-size: 18px;
    letter-spacing: 0.25em;
    text-align: center;
  }
  .btn {
    justify-self: start;
  }
  .log {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 6px;
  }
  .log li {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    flex-wrap: wrap;
    font-size: 13.5px;
  }
  .log small {
    color: var(--faint);
  }
  .log .bad small {
    color: #ff8a80;
  }
</style>
