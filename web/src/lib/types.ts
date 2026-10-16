export type Kind = 'movie' | 'series';

export interface Card {
  id: number;
  kind: Kind;
  title: string;
  originalTitle: string | null;
  year: number | null;
  poster: string | null;
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
}

export interface Home {
  hero: Card[];
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
  progress: Progress | null;
}

export interface TitleDetail {
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
}

export interface Site {
  name: string;
  registration: 'invite' | 'open' | 'closed';
  downloads: boolean;
  version: string;
}

export interface User {
  id: number;
  login: string;
  name: string;
  role: 'host' | 'admin' | 'viewer';
  canDownload: boolean;
}

export interface Me {
  user: User;
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
  position: number;
}
