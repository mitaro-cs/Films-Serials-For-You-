<script lang="ts">
  import ChatList from '$lib/social/ChatList.svelte';
</script>

<div class="page narrow">
  <h1>Чаты</h1>
  <ChatList />
</div>

<style>
  .narrow {
    max-width: 720px;
    margin: 0 auto;
  }
  h1 {
    font-size: clamp(32px, 5vw, 46px);
    font-weight: 800;
    font-style: italic;
    letter-spacing: -0.03em;
    margin: 8px 0 18px;
  }
</style>
