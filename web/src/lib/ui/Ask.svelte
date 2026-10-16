<script lang="ts">
  import { answer, question } from './ask.svelte';
  import Modal from './Modal.svelte';

  let open = $state(false);
  $effect(() => {
    open = !!question.resolve;
  });
  $effect(() => {
    if (!open && question.resolve) answer(false);
  });
</script>

<Modal bind:open title="Подтвердите">
  <p>{question.text}</p>
  <div class="row">
    <button class="btn" onclick={() => answer(false)}>Отмена</button>
    <button class="btn {question.danger ? 'danger' : 'primary'}" onclick={() => answer(true)}>{question.ok}</button>
  </div>
</Modal>

<style>
  p {
    margin: 0 0 20px;
  }
  .row {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }
</style>
