<script lang="ts">
 import {replaceState} from '$app/navigation';
 import {page} from '$app/state';
 import {errorText,post} from '$lib/api';
 import AuthShell from '$lib/ui/AuthShell.svelte';
 import PasswordInput from '$lib/ui/PasswordInput.svelte';
 const token=$derived(new URLSearchParams(page.url.hash.slice(1)).get('token')??'');
 let password=$state(''),confirm=$state(''),error=$state(''),busy=$state(false),done=$state(false);
 async function submit(e:SubmitEvent){e.preventDefault();error='';if(password!==confirm){error='Пароли не совпадают';return;}busy=true;try{await post('/api/auth/reset-password',{token,newPassword:password});password=confirm='';done=true;replaceState(location.pathname,{});}catch(e){error=errorText(e)}finally{busy=false}}
</script>
<AuthShell title={done?'Пароль изменён':'Новый пароль'} subtitle={done?'Войдите в Nova с новым паролем. На других устройствах потребуется войти заново.':'Ссылка действует 15 минут и используется один раз.'}>
{#if done}<a class="btn primary block" href="/login">Войти</a>{:else if !token}<p class="error">В ссылке нет кода восстановления.</p><a class="btn block" href="/forgot-password">Получить новую ссылку</a>{:else}<form onsubmit={submit}><div class="field"><PasswordInput caption="Новый пароль" bind:value={password} autocomplete="new-password" required minlength={6} label="новый пароль"/></div><div class="field"><PasswordInput caption="Повторите пароль" bind:value={confirm} autocomplete="new-password" required minlength={6} label="повтор пароля"/></div>{#if error}<p class="error" role="alert">{error}</p>{/if}<button class="btn primary block" disabled={busy}>{busy?'Сохраняем…':'Сохранить пароль'}</button></form>{/if}
</AuthShell>
