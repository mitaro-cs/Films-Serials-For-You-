<script lang="ts">
 import {api,errorText,post} from '$lib/api';
 import AuthShell from '$lib/ui/AuthShell.svelte';
 let email=$state(''),message=$state(''),error=$state(''),busy=$state(false),enabled=$state(true);
 $effect(()=>{api<{emailRecovery?:boolean}>('/api/site').then(s=>enabled=!!s.emailRecovery,()=>{});});
 async function submit(e:SubmitEvent){e.preventDefault();busy=true;error='';try{const r=await post<{message:string}>('/api/auth/forgot-password',{email});message=r.message;}catch(e){error=errorText(e)}finally{busy=false}}
</script>
<AuthShell title="Восстановить пароль" subtitle="Пароль может сбросить владелец вашей Nova. Почта не нужна.">
<p>Сообщите владельцу свой логин. В приложении хоста нужно открыть «Управление → Люди» и нажать «Сбросить пароль» рядом с вашим именем.</p>
<p class="muted small">Владелец передаст новый пароль лично. Войдите с ним и задайте свой пароль в профиле, на вкладке «Аккаунт».</p>
{#if enabled}<details><summary>Восстановить через настроенную почту</summary>
{#if message}<p role="status">{message}</p>{:else}<form onsubmit={submit}><label class="field"><span>Электронная почта</span><input class="input" type="email" bind:value={email} autocomplete="email" required/></label>{#if !enabled}<p class="muted small">Владелец Nova ещё не настроил отправку писем. Обратитесь к нему, чтобы восстановить доступ.</p>{/if}{#if error}<p class="error" role="alert">{error}</p>{/if}<button class="btn primary block" disabled={busy||!enabled}>{busy?'Отправляем…':'Получить ссылку'}</button><p class="muted small">Если вы не добавляли и не подтверждали почту в аккаунте, обратитесь к владельцу Nova.</p></form>{/if}
</details>{/if}
<a class="btn primary block" href="/login">Вернуться ко входу</a>
</AuthShell>
