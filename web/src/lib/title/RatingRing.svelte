<script lang="ts">
  let { value }: { value: number } = $props();
  const score = $derived(Math.max(0, Math.min(10, value)));
  const color = $derived(score >= 8 ? '#55d488' : score >= 6 ? '#f6cb45' : score >= 4 ? '#f29245' : '#ed6270');
</script>
<div class="rating-ring" style:--score="{score * 10}%" style:--ring={color} aria-label="Рейтинг IMDb {score.toFixed(1)} из 10">
  <span>{score.toFixed(1)}</span><small>IMDb</small>
</div>
<style>
 .rating-ring{width:104px;aspect-ratio:1;border-radius:50%;display:grid;align-content:center;justify-items:center;gap:0;background:radial-gradient(closest-side,#17191c 81%,transparent 83%),conic-gradient(var(--ring) var(--score),#ffffff38 0);font-variant-numeric:tabular-nums;animation:ring-in .6s ease both}
 span{font-size:27px;font-weight:800;line-height:1.1}small{font-size:10px;letter-spacing:.08em;color:#d5d5d5}
 @keyframes ring-in{from{opacity:0;transform:scale(.88)}}@media(prefers-reduced-motion:reduce){.rating-ring{animation:none}}
</style>
