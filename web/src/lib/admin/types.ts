export interface UpdateInfo {
  state: 'idle' | 'checking' | 'latest' | 'available' | 'downloading' | 'installing' | 'error';
  latest: string | null;
  progress: number | null;
  error: string | null;
  checkedAt: number | null;
}

export interface Status {
  version: string;
  /** null — сервер без приложения (`fsfy serve`): обновлять нечего. */
  update: UpdateInfo | null;
  titles: number;
  files: number;
  size: number;
  metaPending: number;
  unprobed: number;
  scan: { running: boolean; at: number | null; error: string | null; last: { files: number; added: number; missing: number; titles: number } | null };
  meta: { running: boolean; error: string | null; done: number };
  ffmpeg: boolean;
  ffprobe: boolean;
  watching: { user: string; title: string; episode: string; position: number; duration: number }[];
  canPick: boolean;
  publicUrl: string | null;
  /** Своя папка для фильмов (null — ещё нет). */
  inbox: string | null;
  /** Копирование перетащенных в окно файлов. */
  import: { running: boolean; filesTotal: number; filesDone: number; bytesTotal: number; bytesDone: number; current: string | null; error: string | null };
}
