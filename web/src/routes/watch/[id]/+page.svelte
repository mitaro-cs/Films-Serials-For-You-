<script lang="ts">
  import { page } from '$app/state';
  import Player from '$lib/player/Player.svelte';

  const id = $derived(Number(page.params.id));
</script>

{#key id}
  <Player fileId={id} />
{/key}
