<script lang="ts">
  let { compact = false }: { compact?: boolean } = $props();
</script>

<span class="logo" class:compact>
  <img src="/logo.svg" width="47" height="24" alt="" />
  {#if !compact}
    <span class="word">Films &amp; Serials <small>For You</small></span>
  {/if}
</span>

<style>
  .logo {
    display: inline-flex;
    align-items: center;
    gap: 9px;
    font-weight: 800;
    font-style: italic;
    font-size: 17px;
    letter-spacing: -0.02em;
    white-space: nowrap;
  }
  img {
    flex: none;
  }
  small {
    font-size: 11px;
    font-weight: 600;
    font-style: normal;
    color: var(--muted);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    margin-left: 2px;
  }
</style>
