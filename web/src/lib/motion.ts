// Анимации интерфейса: включены всегда, выключить — «Профиль → Анимации». Системное
// «уменьшить движение» (на Windows — «Показывать анимацию» выкл.) их больше не гасит: владелец
// просил анимации, а у многих этот флажок выключен по умолчанию. Выключено — класс no-motion
// на <html> (правила — в app.css).
const KEY = 'nova-motion';

export function motionOn(): boolean {
  try {
    return localStorage.getItem(KEY) !== 'off';
  } catch {
    return true;
  }
}

export function setMotion(on: boolean) {
  try {
    if (on) localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, 'off');
  } catch {
    /* без хранилища — до перезагрузки */
  }
  applyMotion(on);
}

export function applyMotion(on = motionOn()) {
  document.documentElement.classList.toggle('no-motion', !on);
}
