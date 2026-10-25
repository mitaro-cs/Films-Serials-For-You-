<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements';
  import Icon from './Icon.svelte';
  let {caption='Пароль',value=$bindable(''),autocomplete='current-password',required=false,minlength=undefined,label='пароль',disabled=false}:{caption?:string;value?:string;autocomplete?:HTMLInputAttributes['autocomplete'];required?:boolean;minlength?:number;label?:string;disabled?:boolean}=$props();
  let visible=$state(false);
  const id=$props.id();
</script>
<span class="password-control"><label for={id}>{caption}</label><span class="password-field"><input {id} class="input" type={visible?'text':'password'} bind:value {autocomplete} {required} {minlength} {disabled} autocapitalize="none" spellcheck="false"/><button type="button" aria-label={`${visible?'Скрыть':'Показать'} ${label}`} aria-pressed={visible} title={visible?'Скрыть пароль':'Показать пароль'} onclick={()=>visible=!visible} {disabled}><Icon name={visible?'eyeoff':'eye'} size={19}/></button></span></span>
<style>
.password-control{display:grid;gap:6px;min-width:0}.password-control>label{font-size:13px;color:var(--muted)}.password-field{display:grid;grid-template-columns:minmax(0,1fr)40px;width:100%;min-width:0;border:1px solid var(--line);border-radius:14px;background:#000;transition:border-color .15s}.password-field:focus-within{border-color:#fff}.input{width:100%;min-width:0;padding-right:12px;border:0;background:transparent}.input:focus{box-shadow:none;outline:0}.input::-ms-reveal,.input::-ms-clear{display:none}button{align-self:stretch;width:40px;display:grid;place-items:center;border:0;border-radius:12px;background:transparent;color:#aaa;cursor:pointer;transition:color .15s,background .15s}button:hover{color:#fff;background:#ffffff10}button:focus-visible{outline:2px solid #fff;outline-offset:-3px}button:active{background:#ffffff20}
</style>
