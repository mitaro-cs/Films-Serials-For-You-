<script lang="ts">
  import Logo from './Logo.svelte';
  let { label = 'Загружаем историю', compact = false }: { label?: string; compact?: boolean } = $props();
</script>
<div class="loading" class:compact role="status" aria-live="polite">
  <div class="symbol" aria-hidden="true"><i></i><i></i><span><Logo /></span></div>
  <span class="label">{label}<b><i></i><i></i><i></i></b></span>
</div>
<style>
  .loading { display: grid; justify-items: center; gap: 20px; padding: 40px 16px; color: #ddd; }
  .symbol { position: relative; width: 90px; height: 90px; display: grid; place-items: center; }
  .symbol > i { position: absolute; inset: 0; border: 1px solid #ffffff18; border-top-color: #fff; border-radius: 50%; animation: orbit 1.8s linear infinite; }
  .symbol > i:nth-child(2) { inset: 10px; border-top-color: transparent; border-right-color: #ffffff70; animation-direction: reverse; animation-duration: 2.4s; }
  .symbol span { width: 38px; overflow: hidden; animation: glow 2s ease-in-out infinite; }
  .symbol :global(.word) { display: none; }
  .label { display: flex; align-items: center; gap: 8px; font-size: 12px; letter-spacing: .02em; }
  b { display: flex; gap: 3px; } b i { width: 3px; height: 3px; background: #fff; border-radius: 50%; animation: glow 1.2s infinite; } b i:nth-child(2) { animation-delay: .2s; } b i:nth-child(3) { animation-delay: .4s; }
  .compact { display: flex; justify-content: flex-start; padding: 12px 0; gap: 10px; } .compact .symbol { width: 24px; height: 24px; } .compact .symbol span { display: none; } .compact .symbol > i:nth-child(2) { inset: 4px; }
  @keyframes orbit { to { transform: rotate(360deg); } } @keyframes glow { 50% { opacity: .3; } }
  :global(.no-motion) .loading i, :global(.no-motion) .symbol span { animation: none; }
</style>
