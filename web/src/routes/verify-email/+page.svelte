<script lang="ts">
 import {replaceState} from '$app/navigation';
 import {page} from '$app/state';
 import {errorText,post} from '$lib/api';
 import AuthShell from '$lib/ui/AuthShell.svelte';
 const token=$derived(new URLSearchParams(page.url.hash.slice(1)).get('token')??'');
 let error=$state(''),busy=$state(false),done=$state(false);
 async function confirm(){busy=true;error='';try{await post('/api/auth/verify-email',{token});done=true;replaceState(location.pathname,{});}catch(e){error=errorText(e)}finally{busy=false}}
</script>
<AuthShell title={done?'Почта подтверждена':'Подтвердить почту'} subtitle={done?'Теперь этот адрес можно использовать для восстановления пароля.':'Подтвердите, что хотите использовать этот адрес для восстановления доступа к Nova.'}>
{#if done}<a class="btn primary block" href="/login">Перейти в Nova</a>{:else if token}<button class="btn primary block" disabled={busy} onclick={confirm}>{busy?'Подтверждаем…':'Подтвердить адрес'}</button>{:else}<p class="error">Ссылка недействительна.</p>{/if}{#if error}<p class="error" role="alert">{error}</p>{/if}
</AuthShell>
