<script lang="ts">
 import {api,errorText,post} from '$lib/api';
 import PasswordInput from '$lib/ui/PasswordInput.svelte';
 import {session} from '$lib/session.svelte';
 let current=$state<{email:string|null;pending:string|null;enabled:boolean}|null>(null);
 let email=$state(''),password=$state(''),busy=$state(false),message=$state(''),error=$state('');
 function load(){api<typeof current>('/api/me/email').then(r=>{current=r;email=r?.pending??r?.email??'';},e=>error=errorText(e));}
 $effect(load);
 async function submit(e:SubmitEvent){e.preventDefault();busy=true;message=error='';try{await post('/api/me/email',{email,password});password='';message='Письмо отправлено. Подтвердите адрес по ссылке в течение 15 минут.';load();}catch(e){error=errorText(e)}finally{busy=false}}
</script>
<form class="card pane" onsubmit={submit} aria-labelledby="email-title">
<h2 id="email-title">Почта для восстановления</h2>
{#if current?.email}<p class="verified">✓ Подтверждена: {current.email}</p>{/if}
{#if current?.pending}<p class="muted small">Ожидает подтверждения: {current.pending}</p>{/if}
<p class="muted small">Ссылка для восстановления отправляется только на подтверждённый адрес.</p>
<label class="field"><span>Электронная почта</span><input class="input" type="email" bind:value={email} autocomplete="email" required/></label>
{#if session.me?.hasPassword}<div class="field"><PasswordInput caption="Текущий пароль Nova для подтверждения изменения" bind:value={password} required label="текущий пароль Nova"/></div>{/if}
{#if current && !current.enabled}<p class="muted small">Отправка писем ещё не настроена владельцем Nova.</p>{/if}
<button class="btn" disabled={busy||!current?.enabled}>Подтвердить почту</button>
{#if message}<p role="status">{message}</p>{/if}{#if error}<p class="error" role="alert">{error}</p>{/if}
</form>
<style>.pane{display:grid;gap:14px;padding:22px;margin-bottom:18px}h2{font-size:17px}.btn{justify-self:start}p{overflow-wrap:anywhere}.verified{font-size:13px;color:#9ce5b8;overflow-wrap:anywhere}</style>
