// Аватары-персонажи Nova: свои мультяшные герои (нарисованы здесь, без чужих изображений).
// Рисунок - в квадрате 64×64, фон-круг рисует Avatar. Номер аватара берётся по кругу (avatar % 12).

export interface Character {
  name: string;
  /** Цвет фона. */
  bg: string;
  /** Внутренность SVG (фигуры поверх фона). */
  art: string;
}

const eyes = (x1: number, x2: number, y: number, r = 3.2, c = '#1b1b1f') =>
  `<circle cx="${x1}" cy="${y}" r="${r}" fill="${c}"/><circle cx="${x2}" cy="${y}" r="${r}" fill="${c}"/>` +
  `<circle cx="${x1 + 1}" cy="${y - 1}" r="1" fill="#fff"/><circle cx="${x2 + 1}" cy="${y - 1}" r="1" fill="#fff"/>`;

export const CHARACTERS: Character[] = [
  {
    name: 'Лиса',
    bg: '#ffb37a',
    art:
      '<path d="M14 18l8 14h20l8-14-10 6H24z" fill="#e8642c"/>' +
      '<path d="M12 30q20-6 40 0-2 20-20 24-18-4-20-24z" fill="#f2793a"/>' +
      '<path d="M22 40q10 10 20 0-4 12-10 12t-10-12z" fill="#fff4ea"/>' +
      eyes(25, 39, 36) +
      '<circle cx="32" cy="45" r="2.6" fill="#1b1b1f"/>',
  },
  {
    name: 'Кот',
    bg: '#9fb4ff',
    art:
      '<path d="M15 16l7 12M49 16l-7 12" stroke="#4b4f63" stroke-width="7" stroke-linecap="round"/>' +
      '<ellipse cx="32" cy="38" rx="20" ry="17" fill="#5b6075"/>' +
      eyes(25, 39, 36, 3.6, '#c8f56a') +
      '<path d="M30 43h4l-2 2.5z" fill="#ff9fb3"/>' +
      '<path d="M32 45.5q-3 3-6 1M32 45.5q3 3 6 1" stroke="#e9ecf5" stroke-width="1.6" fill="none" stroke-linecap="round"/>' +
      '<path d="M13 42h8M43 42h8M14 46l7-1M43 45l7 1" stroke="#e9ecf5" stroke-width="1.2"/>',
  },
  {
    name: 'Панда',
    bg: '#a6e3c2',
    art:
      '<circle cx="17" cy="20" r="7" fill="#222"/><circle cx="47" cy="20" r="7" fill="#222"/>' +
      '<ellipse cx="32" cy="36" rx="20" ry="18" fill="#fafafa"/>' +
      '<ellipse cx="24.5" cy="34" rx="5" ry="6.5" fill="#222" transform="rotate(-20 24.5 34)"/>' +
      '<ellipse cx="39.5" cy="34" rx="5" ry="6.5" fill="#222" transform="rotate(20 39.5 34)"/>' +
      '<circle cx="25.5" cy="33" r="1.8" fill="#fff"/><circle cx="38.5" cy="33" r="1.8" fill="#fff"/>' +
      '<ellipse cx="32" cy="42" rx="3" ry="2.2" fill="#222"/>' +
      '<path d="M28 46q4 3 8 0" stroke="#222" stroke-width="1.8" fill="none" stroke-linecap="round"/>',
  },
  {
    name: 'Сова',
    bg: '#c9a8ff',
    art:
      '<path d="M14 18l8 8M50 18l-8 8" stroke="#6b4a2f" stroke-width="5" stroke-linecap="round"/>' +
      '<ellipse cx="32" cy="38" rx="20" ry="19" fill="#8a5d3b"/>' +
      '<ellipse cx="32" cy="44" rx="11" ry="10" fill="#e7cfa9"/>' +
      '<circle cx="24" cy="32" r="7.5" fill="#fff"/><circle cx="40" cy="32" r="7.5" fill="#fff"/>' +
      eyes(24, 40, 32, 4) +
      '<path d="M30 38l2 4 2-4z" fill="#f2a92a"/>',
  },
  {
    name: 'Медведь',
    bg: '#ffd56b',
    art:
      '<circle cx="17" cy="20" r="7" fill="#8b5a34"/><circle cx="47" cy="20" r="7" fill="#8b5a34"/>' +
      '<circle cx="17" cy="20" r="3.5" fill="#d9a074"/><circle cx="47" cy="20" r="3.5" fill="#d9a074"/>' +
      '<ellipse cx="32" cy="37" rx="20" ry="18" fill="#a06a3f"/>' +
      '<ellipse cx="32" cy="44" rx="9" ry="7" fill="#e6bf94"/>' +
      eyes(25, 39, 34) +
      '<ellipse cx="32" cy="41" rx="3.4" ry="2.4" fill="#2a1a10"/>' +
      '<path d="M29 46q3 2 6 0" stroke="#2a1a10" stroke-width="1.6" fill="none" stroke-linecap="round"/>',
  },
  {
    name: 'Кролик',
    bg: '#ffb3cf',
    art:
      '<ellipse cx="24" cy="15" rx="5" ry="13" fill="#fff"/><ellipse cx="40" cy="15" rx="5" ry="13" fill="#fff"/>' +
      '<ellipse cx="24" cy="15" rx="2.3" ry="9" fill="#ff9dbd"/><ellipse cx="40" cy="15" rx="2.3" ry="9" fill="#ff9dbd"/>' +
      '<ellipse cx="32" cy="40" rx="18" ry="16" fill="#fff"/>' +
      eyes(25, 39, 38) +
      '<path d="M30 44h4l-2 2z" fill="#ff7aa5"/>' +
      '<circle cx="21" cy="45" r="2.6" fill="#ffc6da"/><circle cx="43" cy="45" r="2.6" fill="#ffc6da"/>',
  },
  {
    name: 'Лягушонок',
    bg: '#7fd6ff',
    art:
      '<circle cx="21" cy="22" r="8" fill="#5fc451"/><circle cx="43" cy="22" r="8" fill="#5fc451"/>' +
      '<ellipse cx="32" cy="40" rx="22" ry="15" fill="#5fc451"/>' +
      '<circle cx="21" cy="22" r="5" fill="#fff"/><circle cx="43" cy="22" r="5" fill="#fff"/>' +
      eyes(21, 43, 22, 2.6) +
      '<path d="M20 42q12 9 24 0" stroke="#2f7a28" stroke-width="2.6" fill="none" stroke-linecap="round"/>' +
      '<circle cx="18" cy="38" r="2.6" fill="#ff9b9b" opacity=".7"/><circle cx="46" cy="38" r="2.6" fill="#ff9b9b" opacity=".7"/>',
  },
  {
    name: 'Пингвин',
    bg: '#9ee7e3',
    art:
      '<ellipse cx="32" cy="37" rx="20" ry="20" fill="#26303f"/>' +
      '<ellipse cx="32" cy="41" rx="14" ry="15" fill="#fafafa"/>' +
      eyes(26, 38, 33) +
      '<path d="M28 39q4 4 8 0l-4 5z" fill="#f5a524"/>' +
      '<circle cx="21" cy="41" r="2.4" fill="#ffb3c7" opacity=".8"/><circle cx="43" cy="41" r="2.4" fill="#ffb3c7" opacity=".8"/>',
  },
  {
    name: 'Робот',
    bg: '#8ef0b7',
    art:
      '<path d="M32 8v8" stroke="#5c6b7a" stroke-width="3"/><circle cx="32" cy="8" r="3.4" fill="#ff5c6c"/>' +
      '<rect x="13" y="17" width="38" height="32" rx="9" fill="#b9c6d4"/>' +
      '<rect x="18" y="23" width="28" height="14" rx="6" fill="#1d2632"/>' +
      '<circle cx="25" cy="30" r="3.4" fill="#5ff2ff"/><circle cx="39" cy="30" r="3.4" fill="#5ff2ff"/>' +
      '<rect x="24" y="41" width="16" height="4" rx="2" fill="#5c6b7a"/>' +
      '<rect x="9" y="27" width="4" height="10" rx="2" fill="#8a99a8"/><rect x="51" y="27" width="4" height="10" rx="2" fill="#8a99a8"/>',
  },
  {
    name: 'Пришелец',
    bg: '#b8a6ff',
    art:
      '<path d="M22 14l4 8M42 14l-4 8" stroke="#61d36b" stroke-width="3" stroke-linecap="round"/>' +
      '<circle cx="21" cy="13" r="3" fill="#61d36b"/><circle cx="43" cy="13" r="3" fill="#61d36b"/>' +
      '<path d="M12 34q0-14 20-14t20 14q-2 18-20 20-18-2-20-20z" fill="#7ee08a"/>' +
      '<ellipse cx="23" cy="35" rx="6" ry="7.5" fill="#1b1b1f" transform="rotate(-18 23 35)"/>' +
      '<ellipse cx="41" cy="35" rx="6" ry="7.5" fill="#1b1b1f" transform="rotate(18 41 35)"/>' +
      '<circle cx="24.5" cy="32" r="1.8" fill="#fff"/><circle cx="42.5" cy="32" r="1.8" fill="#fff"/>' +
      '<path d="M28 47q4 2 8 0" stroke="#2f7a38" stroke-width="2" fill="none" stroke-linecap="round"/>',
  },
  {
    name: 'Привидение',
    bg: '#6d6aff',
    art:
      '<path d="M16 54V30q0-16 16-16t16 16v24l-5-4-5 4-6-4-6 4-5-4z" fill="#f4f4ff"/>' +
      eyes(26, 38, 31, 3.4) +
      '<ellipse cx="32" cy="40" rx="3" ry="3.6" fill="#1b1b1f"/>' +
      '<circle cx="22" cy="37" r="2.4" fill="#ffb3cf" opacity=".8"/><circle cx="42" cy="37" r="2.4" fill="#ffb3cf" opacity=".8"/>',
  },
  {
    name: 'Дракончик',
    bg: '#ff9a8a',
    art:
      '<path d="M18 22l-4-10 10 6M46 22l4-10-10 6" fill="#3fa36b"/>' +
      '<path d="M26 14l3 6 3-7 3 7 3-6" fill="#f2c94c"/>' +
      '<ellipse cx="32" cy="37" rx="20" ry="17" fill="#4cc27f"/>' +
      '<ellipse cx="32" cy="45" rx="12" ry="7" fill="#a8ecc2"/>' +
      eyes(24, 40, 33) +
      '<circle cx="28" cy="44" r="1.4" fill="#1d5a3a"/><circle cx="36" cy="44" r="1.4" fill="#1d5a3a"/>' +
      '<path d="M27 49q5 3 10 0" stroke="#1d5a3a" stroke-width="1.6" fill="none" stroke-linecap="round"/>',
  },
];

export const characterOf = (avatar: number) => CHARACTERS[((avatar % CHARACTERS.length) + CHARACTERS.length) % CHARACTERS.length];
