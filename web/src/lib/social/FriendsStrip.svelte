<script lang="ts">
  import type { FriendMark } from '$lib/types';
  import Avatar from '$lib/ui/Avatar.svelte';

  // Друзья у фильма: кто смотрит, кто посмотрел, кто собирается. Лица внахлёст и подпись;
  // нажать на лицо - страница человека.
  let { friends }: { friends: FriendMark[] } = $props();

  const WORD = { watching: 'смотрит', watched: 'посмотрел(а)', want: 'хочет посмотреть' } as const;
  const lead = $derived(friends[0]);
  const text = $derived.by(() => {
    if (!lead) return '';
    const first = lead.person.name.split(/\s+/)[0];
    const rest = friends.length - 1;
    return rest > 0 ? `${first} и ещё ${rest} - ${WORD[lead.status]}` : `${first} ${WORD[lead.status]}`;
  });
</script>

{#if friends.length}
  <div class="fs" role="group" aria-label="Друзья у этого фильма">
    <span class="faces">
      {#each friends.slice(0, 5) as f, i (f.person.id)}
        <a href="/people/{f.person.id}" class="face {f.status}" style:--i={i} title="{f.person.name} - {WORD[f.status]}">
          <Avatar avatar={f.person.avatar} photo={f.person.photo} size={30} />
        </a>
      {/each}
    </span>
    <span class="txt">{text}</span>
  </div>
{/if}

<style>
  .fs {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    max-width: 100%;
    font-size: 13px;
    color: rgba(255, 255, 255, 0.8);
  }
  .faces {
    display: flex;
    flex: none;
  }
  .face {
    position: relative;
    display: block;
    border-radius: 50%;
    box-shadow: 0 0 0 2px #000;
    animation: in 0.45s cubic-bezier(0.2, 0.9, 0.3, 1.4) both;
    animation-delay: calc(var(--i) * 60ms + 200ms);
    transition: transform 0.2s cubic-bezier(0.2, 0.9, 0.3, 1.5);
  }
  .face:not(:first-child) {
    margin-left: -8px;
  }
  .face:hover {
    transform: translateY(-3px) scale(1.08);
    z-index: 1;
  }
  /* Смотрит сейчас - зелёная точка, как «в сети». */
  .face.watching::after {
    content: '';
    position: absolute;
    right: -1px;
    bottom: -1px;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--ok);
    box-shadow: 0 0 0 2px #000;
  }
  .txt {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  @keyframes in {
    from {
      opacity: 0;
      transform: scale(0.5);
    }
  }
</style>
