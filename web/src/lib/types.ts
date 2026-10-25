export type Kind = 'movie' | 'series';

export interface Card {
  id: number;
  kind: Kind;
  title: string;
  originalTitle: string | null;
  year: number | null;
  poster: string | null;
  posterLarge: string | null;
  backdrop: string | null;
  logo: string | null;
  rating: number | null;
  genres: string[];
  overview: string | null;
  runtime: number | null;
  age: string | null;
  addedAt: number;
  files: number;
  seasons: number;
  inList: boolean;
  /** -1 - «не для меня», 1 - «нравится», 2 - «обожаю». */
  myRating: -1 | 1 | 2 | null;
  matchPct: number | null;
  /** Для билета: первая страна, режиссёр, цвет низа карточки по постеру. */
  country: string | null;
  director: string | null;
  accent: string | null;
  /** Кадр 640 px для широкой карточки. */
  backdropSmall: string | null;
  status: 'want' | 'watching' | 'watched' | 'dropped' | null;
  /** Когда отмечено «Просмотрено». */
  watchedAt: number | null;
  /** Где остановился (у начатого). */
  resume: Next | null;
}

export interface Library {
  items: Card[];
  counts: Record<'all' | 'want' | 'watching' | 'watched', number>;
  /** Начатое, последнее - первым. */
  recent: number[];
}

export interface HeroCard extends Card {
  preview: { fileId: number; start: number } | null;
}

export interface Next {
  fileId: number;
  label: string;
  position: number;
  duration: number;
}

export interface ContinueItem extends Card {
  next: Next;
  still: string | null;
}

export interface Row {
  key: string;
  title: string;
  items: Card[];
  style: 'poster' | 'top';
}

export interface Home {
  hero: HeroCard[];
  continue: ContinueItem[];
  rows: Row[];
}

export interface Person {
  name: string;
  role: string | null;
  job: 'director' | 'writer' | 'creator' | 'actor' | string;
  photo: string | null;
}

export interface Progress {
  position: number;
  duration: number;
  done: boolean;
}

export interface FileItem {
  id: number;
  season: number | null;
  episode: number | null;
  episodeEnd: number | null;
  name: string | null;
  overview: string | null;
  still: string | null;
  airDate: string | null;
  duration: number | null;
  size: number;
  fileName: string;
  addedAt: number;
  progress: Progress | null;
  /** Оценка серии на IMDb. */
  rating: number | null;
}

/** Друг и его отметка у фильма («смотрит», «просмотрел», «хочет»). */
/** Человек в общении (без роли в фильме). */
export type Who = { id: number; name: string; avatar: number; photo: string | null };

export interface FriendMark {
  person: Who;
  status: 'want' | 'watching' | 'watched';
}

/** Что человек смотрит сейчас (или что смотрят друзья - тогда с `person`). */
export interface NowItem {
  at: number;
  label: string;
  card?: Card;
  ext?: ExtCard;
  person?: Who;
}

/** Серия сериала из ленты IMDb. */
export interface ExtEpisode {
  season: number;
  episode: number;
  name: string | null;
  overview: string | null;
  still: string | null;
  airDate: string | null;
  runtime: number | null;
  rating: number | null;
  seen: boolean;
}

export interface TitleDetail {
  metadataPending?: boolean;
  card: Card;
  tagline: string | null;
  countries: string[];
  languages: string[];
  aka: string[];
  people: Person[];
  votes: number | null;
  releaseDate: string | null;
  gallery: string[];
  imdbId: string | null;
  provider: string | null;
  files: FileItem[];
  seasons: number[];
  next: Next | null;
  inList: boolean;
  similar: Card[];
  canDownload: boolean;
  friends: FriendMark[];
}

export interface Site {
  metadataLanguage?: 'ru' | 'source';
  name: string;
  registration: 'invite' | 'open' | 'closed';
  downloads: boolean;
  version: string;
}

export interface Profile {
  id: number;
  name: string;
  avatar: number;
  kids: boolean;
  autoplay: boolean;
  photo: string | null;
}

/** Автор отзыва, участник, собеседник. */
export interface Person {
  id: number;
  name: string;
  avatar: number;
  photo: string | null;
}

export interface Review {
  id: number;
  author: Person;
  /** 1..10 - половинки звезды. */
  stars: number | null;
  body: string;
  createdAt: number;
  editedAt: number | null;
  likes: number;
  liked: boolean;
  mine: boolean;
}

export interface Reviews {
  average: number | null;
  votes: number;
  total: number;
  items: Review[];
  canWrite: boolean;
}

export type Relation = 'none' | 'friend' | 'incoming' | 'outgoing';

export interface Member extends Person {
  relation: Relation;
  watched: number;
  reviews: number;
}

export interface MiniTitle {
  id: number;
  kind: Kind;
  title: string;
  year: number | null;
  poster: string | null;
}

export interface Message {
  id: number;
  mine: boolean;
  body: string;
  createdAt: number;
  read: boolean;
  title: MiniTitle | null;
}

export interface User {
  id: number;
  login: string;
  name: string;
  role: 'host' | 'admin' | 'viewer';
  canDownload: boolean;
  profile: Profile;
}

export interface Me {
  user: User;
  /** Любимые фильмы (до 5): «t:12» или «x:tt…». */
  favorites?: string[];
  hasPassword: boolean;
  manage: boolean;
  window: boolean;
  site: Site;
}

export interface Track {
  index: number;
  label: string;
  lang: string | null;
  default: boolean;
}

export interface Sub {
  id: string;
  label: string;
  lang: string | null;
  forced: boolean;
}

export interface PlayInfo {
  play: {
    direct: boolean;
    remux: boolean;
    transcode: boolean;
    hevc: boolean;
    duration: number | null;
    width: number | null;
    height: number | null;
    audio: Track[];
    subs: Sub[];
  };
  titleId: number;
  kind: Kind;
  title: string;
  episode: string;
  prev: number | null;
  next: number | null;
  nextInfo: { label: string; still: string | null } | null;
  season: number | null;
  position: number;
  markers: { intro: [number, number] | null; credits: number | null; manual: boolean };
  trickplay: Trickplay | null;
  hls: boolean;
  /** Мбит/с файла - сколько нужно для «Оригинала». */
  bitrate?: number | null;
}

export interface Trickplay {
  interval: number;
  width: number;
  height: number;
  cols: number;
  rows: number;
  count: number;
  sheets: number;
}

/** Человек в совместном списке. */
export type Friend = Pick<Member, 'id' | 'name' | 'avatar' | 'photo'>;

/** Совместный список: «смотрим вместе». */
export interface SharedList {
  id: number;
  name: string;
  mine: boolean;
  count: number;
  watched: number;
  members: Friend[];
  art: string[];
}

export interface SharedItem {
  /** С сервиса или из ленты IMDb - одно из двух. */
  card?: Card;
  ext?: ExtCard;
  groupId: number | null;
  addedAt: number;
  watchedAt: number | null;
  addedBy: Friend | null;
}

export interface SharedListFull {
  id: number;
  name: string;
  mine: boolean;
  members: Friend[];
  items: SharedItem[];
  groups: { id: number; name: string }[];
}

/** Фильм или сериал из ленты IMDb (может не быть на сервисе). */
export interface ExtCard {
  id: string;
  kind: Kind;
  title: string;
  original: string | null;
  year: number | null;
  rating: number | null;
  poster: string | null;
  genres: string[];
  /** Есть на сервисе - id фильма у нас. */
  local: number | null;
  mark: 'want' | 'watching' | 'watched' | 'dropped' | null;
  wished: boolean;
}

export interface ExtDetail {
  refreshing?: boolean;
  card: ExtCard;
  overview: string | null;
  runtime: number | null;
  votes: number | null;
  /** Большой кадр и постер - копии у нас. */
  backdrop: string | null;
  posterLarge: string | null;
  tagline: string | null;
  age: string | null;
  releaseDate: string | null;
  countries: string[];
  languages: string[];
  aka: string[];
  people: { name: string; job: string; role: string | null; photo: string | null }[];
  gallery: string[];
  similar: ExtCard[];
  localCard: Card | null;
  lists: { id: number; name: string; has: boolean }[];
  links: VideoLink[];
  /** Номера сезонов (у сериала). */
  seasons: number[];
  friends: FriendMark[];
}

/** Видео по ссылке: Rutube, VK, YouTube (их плеер) или прямой файл (наш). */
export interface VideoLink {
  season?: number | null;
  episode?: number | null;
  id: number;
  kind: 'rutube' | 'vk' | 'youtube' | 'direct' | 'site';
  url: string;
  embed: string | null;
  label: string;
  thumb: string | null;
  duration: number | null;
  titleId: number | null;
  extId: string | null;
  addedBy: number | null;
  author: string;
  addedAt: number;
  /** Где остановился этот зритель. */
  progress: { position: number; duration: number } | null;
}

/** Найденное на Rutube/VK - ещё не сохранено. */
export interface FoundVideo {
  site: 'rutube' | 'vk' | 'dailymotion' | 'archive' | 'peertube' | 'commons';
  url: string;
  label: string;
  thumb: string | null;
  thumbUrl: string | null;
  duration: number | null;
  author: string | null;
}

/** Элемент «Буду смотреть»/«Просмотрел»: с сервиса или из ленты. */
export interface MarkItem {
  at: number;
  card?: Card;
  ext?: ExtCard;
}

export type Marks = { want: MarkItem[]; watching: MarkItem[]; watched: MarkItem[]; favorites?: { key: string; card?: Card; ext?: ExtCard }[] };

export interface ResumeItem { key: string; href: string; position: number; duration: number; label: string; at: number; art: string | null; card?: Card; ext?: ExtCard; season?: number; episode?: number; }

export interface ProfileStats {
  hours: number;
  movies: number;
  episodes: number;
  series: number;
  reviews: number;
  friends: number;
  outside: number;
  watched: number;
  genres: { genre: string; count: number }[];
  months: number[];
  monthLabels?: string[];
  /** Режиссёры того, что смотрел, - чаще всего первыми. */
  directors: { name: string; count: number }[];
}

export interface ProfileDecor {
  cover: string | null;
  /** Обложка: лента просмотренного или своя картинка. */
  coverKind: 'ribbon' | 'image';
  directors: string[];
  /** Постеры просмотренного - для ленты на обложке. */
  ribbon: string[];
  accent: string | null;
  bio: string | null;
  favorites: { key: string; card?: Card; ext?: ExtCard }[];
  /** Показывать друзьям, что смотрю. */
  showActivity: boolean;
  /** Фото любимых режиссёров (что уже известно). */
  directorsInfo?: { name: string; photo: string | null }[];
}

/** Совместный просмотр: место в видео у всех в комнате. */
export interface RoomState {
  revision?: number;
  fileId: number;
  /** Видео по ссылке (фильм из Обзора) - тогда fileId = 0. */
  linkId: number | null;
  playing: boolean;
  /** Секунды на момент `at` (мс); при `playing` - идёт дальше. */
  position: number;
  at: number;
  by: string;
}

export interface RoomChat {
  from: Who;
  body: string;
  at: number;
}

export type RoomEvent =
  | { type: 'snapshot'; id: string; ownerId: number; titleId: number; state: RoomState; members: Who[]; chat: RoomChat[]; voice: number[] }
  | { type: 'voice'; ids: number[] }
  | { type: 'signal'; from: number; to: number; data: { type: 'offer' | 'answer'; sdp: RTCSessionDescriptionInit } | { type: 'ice'; candidate: RTCIceCandidateInit } }
  | { type: 'state'; state: RoomState }
  | { type: 'members'; members: Who[] }
  | ({ type: 'chat' } & RoomChat)
  | { type: 'react'; from: Who; emoji: string; at: number };
