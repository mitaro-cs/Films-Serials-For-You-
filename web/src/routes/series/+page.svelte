<script lang="ts">
  import { goto } from '$app/navigation';

  // Старые ссылки (0.2) ведут в каталог-билеты.
  $effect(() => {
    goto('/?tab=series', { replaceState: true });
  });
</script>
