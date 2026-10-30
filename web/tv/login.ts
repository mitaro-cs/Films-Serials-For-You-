// Вход: логин и пароль (и код из приложения, если включён второй шаг).

import { ApiError, errorText, post } from './api';
import { forgetLibrary, go, loadMe, type Screen } from './app';
import { esc } from './util';

export const loginScreen: Screen = (root, route) => {
  const next = route.query.get('next') || '/';
  root.innerHTML =
    '<section class="login"><div class="logo big">Nova</div>' +
    '<p class="hint">Войдите так же, как на телефоне или компьютере</p>' +
    '<input id="l" class="f field" data-first data-k="l" type="text" placeholder="Логин" autocomplete="off" autocapitalize="off">' +
    '<input id="p" class="f field" data-k="p" type="password" placeholder="Пароль" autocomplete="off">' +
    '<input id="c" class="f field" data-k="c" type="text" inputmode="numeric" placeholder="Код из приложения" autocomplete="off" style="display:none">' +
    '<a class="f btn pri" data-k="in" id="in">Войти</a><p class="err" id="err"></p></section>';
  const q = (id: string) => root.querySelector('#' + id) as HTMLInputElement;
  const err = root.querySelector('#err') as HTMLElement;
  let busy = false;
  const submit = async () => {
    if (busy) return;
    busy = true;
    err.textContent = '';
    try {
      await post('/api/auth/login', { login: q('l').value, password: q('p').value, code: q('c').value || undefined });
      forgetLibrary();
      await loadMe();
      go(next, true);
    } catch (e) {
      if (e instanceof ApiError && (e.code === 'totp_required' || e.code === 'bad_totp')) q('c').style.display = '';
      err.innerHTML = esc(errorText(e));
    }
    busy = false;
  };
  const onClick = (e: Event) => {
    if ((e.target as HTMLElement).id === 'in') submit();
  };
  root.addEventListener('click', onClick);
  return () => root.removeEventListener('click', onClick);
};
