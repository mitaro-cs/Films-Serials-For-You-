<script lang="ts">
  import { goto } from '$app/navigation';
  import { fade, fly } from 'svelte/transition';
  import { motionOn } from '$lib/motion';
  import Icon from './Icon.svelte';

  // Экскурсия по сервису: несколько экранов с анимацией, у каждого - «Показать» (открывает раздел).
  // Запуск: сама после знакомства у нового человека и в любой момент - событие `nova-tour`
  // (кнопки «Экскурсия по Nova» в меню и в профиле).
  type Step = { icon: string; title: string; text: string; href?: string; show?: string };
  const STEPS: Step[] = [
    { icon: 'play', title: 'Добро пожаловать в Nova', text: 'Ваш кинозал: фильмы и сериалы с компьютера хозяина, друзья рядом, всё открывается в браузере и на телевизоре. Покажем, где что лежит - это минута.' },
    { icon: 'film', title: 'Главная и разделы', text: 'На главной - то, что вы смотрите, новое и подборки по вкусу. «Фильмы», «Сериалы» и «Аниме» - только своё; статусы «Хочу», «Смотрю», «Просмотрено» собирают всё в одном месте.', href: '/', show: 'Открыть главную' },
    { icon: 'globe', title: 'Обзор - всё кино мира', text: 'Чарты, новинки и поиск по IMDb. Нашли то, чего нет в библиотеке - отметьте «Хочу» или попросите хозяина добавить; фильм появится сам.', href: '/discover', show: 'Открыть Обзор' },
    { icon: 'list', title: 'Списки и отметки', text: 'Ставьте оценки, пишите отзывы, собирайте общие списки с друзьями: «Что посмотреть в пятницу», «Ужасы на Хэллоуин». Любимые фильмы живут в профиле.', href: '/lists', show: 'Открыть списки' },
    { icon: 'party', title: 'Смотрим вместе', text: 'Комната на двоих и больше: видео идёт синхронно, есть чат, реакции и голос. Позовите друга сообщением со ссылкой.', href: '/people', show: 'Найти друзей' },
    { icon: 'captions', title: 'Плеер', text: 'Качество само подстраивается под интернет. Пробел - пауза, стрелки - перемотка, S - пропустить заставку, F - во весь экран. Субтитры и дорожки - в меню плеера.' },
    { icon: 'user', title: 'Профиль', text: 'Статистика просмотров, обложка, цвет, любимые режиссёры. Тут же можно выключить анимации и снова запустить эту экскурсию.', href: '/profile', show: 'Открыть профиль' },
    { icon: 'tv', title: 'И на телевизоре', text: 'Откройте адрес Nova в браузере телевизора - сервер покажет интерфейс под пульт. Войти можно тем же логином и паролем.' },
  ];

  let open = $state(false);
  let step = $state(0);
  let dir = $state(1);
  const last = $derived(step === STEPS.length - 1);
  const s = $derived(STEPS[step]);
  const d = $derived(motionOn() ? 280 : 0);

  $effect(() => {
    const start = () => {
      step = 0;
      dir = 1;
      open = true;
    };
    window.addEventListener('nova-tour', start);
    return () => window.removeEventListener('nova-tour', start);
  });

  function go(to: number) {
    dir = to > step ? 1 : -1;
    step = Math.max(0, Math.min(STEPS.length - 1, to));
  }
  function close() {
    open = false;
  }
  function show() {
    const href = s.href;
    close();
    if (href) goto(href);
  }
  function key(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === 'ArrowRight') go(step + 1);
    else if (e.key === 'ArrowLeft') go(step - 1);
    else if (e.key === 'Escape') close();
    else return;
    e.preventDefault();
  }
</script>

<svelte:window onkeydown={key} />

{#if open}
  <div class="veil" role="dialog" aria-modal="true" aria-label="Экскурсия по Nova" transition:fade={{ duration: d }}>
    <div class="card" transition:fly={{ y: 24, duration: d + 60 }}>
      <div class="top">
        <span>Экскурсия · {step + 1} из {STEPS.length}</span>
        <button class="skip" onclick={close}>Закрыть</button>
      </div>
      <div class="bar"><i style:width="{((step + 1) / STEPS.length) * 100}%"></i></div>
      <div class="stage">
        {#key step}
          <div class="slide" in:fly={{ x: 46 * dir, duration: d }} out:fade={{ duration: d / 2 }}>
            <div class="art" aria-hidden="true">
              <span class="ring r1"></span><span class="ring r2"></span>
              <span class="badge"><Icon name={s.icon as never} size={44} /></span>
            </div>
            <h2>{s.title}</h2>
            <p>{s.text}</p>
          </div>
        {/key}
      </div>
      <div class="dots" role="tablist" aria-label="Шаги">
        {#each STEPS as _, i (i)}<button role="tab" aria-selected={i === step} aria-label="Шаг {i + 1}" class:on={i === step} onclick={() => go(i)}></button>{/each}
      </div>
      <div class="actions">
        {#if step > 0}<button class="btn" onclick={() => go(step - 1)}>Назад</button>{/if}
        <span class="grow"></span>
        {#if s.href}<button class="btn" onclick={show}>{s.show}</button>{/if}
        <button class="btn primary" onclick={() => (last ? close() : go(step + 1))}>{last ? 'Готово' : 'Дальше'}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .veil {
    position: fixed;
    inset: 0;
    z-index: 2300;
    display: grid;
    place-items: center;
    padding: 18px;
    background: #000000d9;
  }
  .card {
    width: min(620px, 100%);
    padding: clamp(20px, 4vw, 34px);
    border: 1px solid #ffffff24;
    border-radius: 26px;
    background: #17171a;
    color: #fff;
    box-shadow: 0 40px 120px -30px #000;
  }
  .top {
    display: flex;
    justify-content: space-between;
    align-items: center;
    color: #a1a1aa;
    font-size: 13px;
  }
  .skip {
    border: 0;
    background: none;
    color: #a1a1aa;
    font: inherit;
    cursor: pointer;
  }
  .skip:hover {
    color: #fff;
  }
  .bar {
    height: 3px;
    margin: 14px 0 6px;
    border-radius: 2px;
    background: #ffffff1f;
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: #fff;
    transition: width 0.4s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .stage {
    display: grid;
    min-height: 330px;
  }
  .slide {
    grid-area: 1 / 1;
    display: grid;
    align-content: start;
    justify-items: center;
    text-align: center;
    padding-top: 18px;
  }
  .art {
    position: relative;
    display: grid;
    place-items: center;
    width: 128px;
    height: 128px;
    margin-bottom: 20px;
  }
  .badge {
    display: grid;
    place-items: center;
    width: 84px;
    height: 84px;
    border-radius: 50%;
    background: #fff;
    color: #111;
    animation: badge-in 0.55s cubic-bezier(0.2, 0.9, 0.3, 1.4) both;
  }
  .ring {
    position: absolute;
    inset: 0;
    border: 1.5px solid #ffffff3d;
    border-radius: 50%;
    animation: ring 2.6s ease-out infinite;
  }
  .r2 {
    animation-delay: 1.3s;
  }
  @keyframes ring {
    from {
      opacity: 0.9;
      transform: scale(0.65);
    }
    to {
      opacity: 0;
      transform: scale(1.15);
    }
  }
  @keyframes badge-in {
    from {
      opacity: 0;
      transform: scale(0.6) rotate(-12deg);
    }
  }
  h2 {
    margin: 0 0 10px;
    font-size: clamp(22px, 4.6vw, 30px);
    letter-spacing: -0.02em;
  }
  p {
    margin: 0;
    max-width: 46ch;
    color: #b4b4bb;
    line-height: 1.6;
    font-size: 15px;
  }
  .dots {
    display: flex;
    justify-content: center;
    gap: 8px;
    margin: 8px 0 18px;
  }
  .dots button {
    width: 8px;
    height: 8px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: #ffffff33;
    cursor: pointer;
    transition:
      width 0.3s,
      background 0.3s;
  }
  .dots button.on {
    width: 24px;
    border-radius: 4px;
    background: #fff;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .grow {
    flex: 1;
  }
  :global(.no-motion) :is(.badge, .ring) {
    animation: none;
  }
</style>
