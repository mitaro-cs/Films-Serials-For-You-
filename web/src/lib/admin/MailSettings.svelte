<script lang="ts">
 import {api,errorText,post,put} from '$lib/api';
 import PasswordInput from '$lib/ui/PasswordInput.svelte';
 let config=$state({enabled:false,host:'smtp.gmail.com',port:465,tls:'tls',user:'',from:'',base:'',passwordSet:false});
 let password=$state(''),recipient=$state(''),message=$state(''),error=$state(''),busy=$state(false);
 $effect(()=>{api<typeof config>('/api/admin/mail').then(r=>config=r,e=>error=errorText(e));});
 async function save(e:SubmitEvent){e.preventDefault();busy=true;error=message='';try{await put('/api/admin/mail',{...config,password:password||undefined});password='';config=await api('/api/admin/mail');message='Настройки почты сохранены';}catch(e){error=errorText(e)}finally{busy=false}}
 async function test(e:SubmitEvent){e.preventDefault();busy=true;error=message='';try{await post('/api/admin/mail/test',{email:recipient});message='Тестовое письмо отправлено. Проверьте входящие и спам.';}catch(e){error=errorText(e)}finally{busy=false}}
</script>
<section class="card pane" aria-labelledby="mail-title">
<h2 id="mail-title">Почта и восстановление доступа</h2>
<p class="muted small">Gmail: включите двухэтапную аутентификацию и создайте <a class="text-link" href="https://myaccount.google.com/apppasswords" target="_blank" rel="noopener noreferrer">пароль приложения</a>. Его вводят здесь один раз.</p>
<form onsubmit={save}>
<label class="switch"><input type="checkbox" bind:checked={config.enabled}/> Отправлять письма для подтверждения почты и восстановления пароля</label>
<label class="field"><span>Gmail отправителя</span><input class="input" type="email" bind:value={config.user} autocomplete="username" required={config.enabled}/></label>
<div class="field"><PasswordInput caption="Пароль приложения Gmail" bind:value={password} autocomplete="off" label="пароль приложения Gmail"/><small class="muted">{config.passwordSet?'Пароль сохранён. Оставьте поле пустым, чтобы сохранить прежний.':'Используйте пароль приложения Google, а не пароль аккаунта.'}</small></div>
<details><summary>Параметры SMTP и адрес Nova</summary>
<label class="field"><span>SMTP-сервер</span><input class="input" bind:value={config.host} required/></label>
<label class="field"><span>Порт</span><input class="input" type="number" bind:value={config.port} min="1" max="65535" required/></label>
<label class="field"><span>Шифрование</span><select class="input" bind:value={config.tls}><option value="tls">TLS · 465</option><option value="starttls">STARTTLS · 587</option></select></label>
<label class="field"><span>Адрес отправителя, если отличается</span><input class="input" type="email" bind:value={config.from}/></label>
<label class="field"><span>Публичный адрес Nova для ссылок в письмах</span><input class="input" type="url" bind:value={config.base} placeholder="https://nova.example"/><small class="muted">Если оставить пустым, используется текущая ссылка доступа Nova. Для писем друзьям нужен адрес, доступный им из интернета.</small></label>
</details>
<button class="btn" disabled={busy}>Сохранить почту</button>
</form>
<form class="test" onsubmit={test}><label class="field"><span>Куда отправить тестовое письмо</span><input class="input" type="email" bind:value={recipient} required/></label><button class="btn" disabled={busy||!config.enabled}>Проверить отправку</button></form>
{#if message}<p role="status">{message}</p>{/if}{#if error}<p class="error" role="alert">{error}</p>{/if}
</section>
<style>.pane{display:grid;gap:16px;padding:22px;margin-top:20px;scroll-margin-top:calc(var(--header) + 20px)}h2{font-size:18px;line-height:1.3}p{overflow-wrap:anywhere}form{display:grid;gap:14px}details summary{cursor:pointer;margin-bottom:12px}details .field{margin-bottom:12px}.test{margin-top:24px;padding-top:20px;border-top:1px solid var(--line)}.switch{display:flex;gap:10px;align-items:flex-start}.text-link{text-decoration:underline}</style>
