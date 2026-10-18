<script lang="ts">
  let { compact = false }: { compact?: boolean } = $props();
</script>

<span class="logo" class:compact>
  <img src="/logo.svg" width="47" height="24" alt="" />
  {#if !compact}
    <span class="word">Nova</span>
  {/if}
</span>

<style>
  .logo {
    display: inline-flex;
    align-items: center;
    gap: 9px;
    font-weight: 800;
    font-size: 17px;
    letter-spacing: -0.02em;
    white-space: nowrap;
  }
  img {
    flex: none;
  }
</style>
