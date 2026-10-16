<script lang="ts">
  let { compact = false }: { compact?: boolean } = $props();
</script>

<span class="logo" class:compact>
  <svg viewBox="0 0 32 32" width="30" height="30" aria-hidden="true">
    <defs>
      <linearGradient id="lg" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stop-color="#ff4f7b" />
        <stop offset="1" stop-color="#6d5cff" />
      </linearGradient>
    </defs>
    <circle cx="16" cy="16" r="15" fill="url(#lg)" />
    <path d="M13 10.5v11l9-5.5z" fill="#fff" />
  </svg>
  {#if !compact}
    <span class="word">Films<i>&amp;</i>Serials <small>For You</small></span>
  {/if}
</span>

<style>
  .logo {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    font-weight: 800;
    font-size: 17px;
    letter-spacing: -0.02em;
    white-space: nowrap;
  }
  i {
    font-style: normal;
    background: var(--accent-grad);
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
    margin: 0 1px;
  }
  small {
    font-size: 11px;
    font-weight: 600;
    color: var(--muted);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    margin-left: 2px;
  }
</style>
