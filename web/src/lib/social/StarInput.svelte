<script lang="ts">
  // Оценка звёздами с половинками: значение 1..10 (= 0,5..5 звёзд). Нажатие на левую
  // половину звезды — половинка; повторное нажатие на ту же оценку — снять. Стрелки — по половинке.
  let {
    value = null,
    size = 26,
    onchange,
    label = 'Оценка',
  }: { value?: number | null; size?: number; onchange?: (v: number | null) => void; label?: string } = $props();

  let hover = $state<number | null>(null);
  const shown = $derived(hover ?? value ?? 0);

  function at(e: PointerEvent, i: number): number {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    return i * 2 - (e.clientX - r.left < r.width / 2 ? 1 : 0);
  }

  function set(v: number | null) {
    onchange?.(v === value ? null : v);
  }

  function key(e: KeyboardEvent) {
    const v = value ?? 0;
    if (e.key === 'ArrowRight' || e.key === 'ArrowUp') onchange?.(Math.min(10, v + 1));
    else if (e.key === 'ArrowLeft' || e.key === 'ArrowDown') onchange?.(v <= 1 ? null : v - 1);
    else return;
    e.preventDefault();
  }

  const STAR = 'M12 2.8l2.8 5.9 6.4.8-4.7 4.4 1.2 6.4L12 17.2l-5.7 3.1 1.2-6.4-4.7-4.4 6.4-.8z';
</script>

<div
  class="stars"
  role="slider"
  tabindex="0"
  aria-label={label}
  aria-valuemin={0}
  aria-valuemax={5}
  aria-valuenow={(value ?? 0) / 2}
  aria-valuetext={value ? `${String(value / 2).replace('.', ',')} из 5` : 'нет оценки'}
  onkeydown={key}
  onpointerleave={() => (hover = null)}
>
  {#each [1, 2, 3, 4, 5] as i (i)}
    <button
      type="button"
      tabindex="-1"
      aria-hidden="true"
      onpointermove={(e) => (hover = at(e, i))}
      onclick={(e) => set(at(e as unknown as PointerEvent, i))}
      style:--fill={shown >= i * 2 ? '100%' : shown === i * 2 - 1 ? '50%' : '0%'}
    >
      <svg width={size} height={size} viewBox="0 0 24 24">
        <path d={STAR} class="bg" />
        <path d={STAR} class="fg" />
      </svg>
    </button>
  {/each}
</div>

<style>
  .stars {
    display: inline-flex;
    gap: 2px;
    border-radius: 8px;
    outline-offset: 4px;
  }
  button {
    padding: 0;
    border: 0;
    background: none;
    cursor: pointer;
    transition: transform 0.15s cubic-bezier(0.2, 0.9, 0.3, 1.4);
  }
  button:hover {
    transform: scale(1.15);
  }
  .bg {
    fill: rgba(255, 255, 255, 0.16);
  }
  .fg {
    fill: #fff;
    clip-path: inset(0 calc(100% - var(--fill)) 0 0);
    transition: clip-path 0.12s;
  }
</style>
