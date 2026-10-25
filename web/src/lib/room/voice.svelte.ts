import { post } from '$lib/api';

// Голосовой чат комнаты: браузеры говорят друг с другом напрямую (WebRTC), сервер Nova только
// передаёт сигналы (предложение, ответ, адреса) через поток событий комнаты. Новенький звонит всем,
// кто уже в чате; встречные звонки разводит «вежливый» (у кого id больше - уступает).
// Адрес снаружи узнаём у общих STUN-серверов - иначе через туннель браузеры друг друга не найдут.

const ICE: RTCIceServer[] = [{ urls: 'stun:stun.l.google.com:19302' }, { urls: 'stun:stun.cloudflare.com:3478' }];

type Signal =
  | { type: 'offer' | 'answer'; sdp: RTCSessionDescriptionInit }
  | { type: 'ice'; candidate: RTCIceCandidateInit };

export class Voice {
  on = $state(false);
  muted = $state(false);
  busy = $state(false);
  error = $state('');
  /** Кто в голосовом чате (id профилей). */
  ids = $state<number[]>([]);
  /** Кто сейчас говорит. */
  speaking = $state<Record<number, boolean>>({});

  private peers = new Map<number, RTCPeerConnection>();
  private audios = new Map<number, HTMLAudioElement>();
  private stops = new Map<number, () => void>();
  private stream: MediaStream | null = null;
  private ctx: AudioContext | null = null;

  constructor(
    private room: string,
    private me: number,
  ) {}

  private send(to: number, data: Signal) {
    post(`/api/rooms/${this.room}/signal`, { to, data }).catch(() => {});
  }

  async join() {
    this.error = '';
    if (!navigator.mediaDevices?.getUserMedia) {
      this.error = 'Микрофон работает только по защищённой ссылке (https) или в окне на этом компьютере';
      return;
    }
    this.busy = true;
    try {
      this.stream = await navigator.mediaDevices.getUserMedia({
        audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true },
      });
    } catch {
      this.error = 'Нет доступа к микрофону - разрешите его в браузере';
      this.busy = false;
      return;
    }
    this.on = true;
    this.muted = false;
    this.meter(this.me, this.stream);
    const others = this.ids.filter((i) => i !== this.me);
    await post(`/api/rooms/${this.room}/voice`, { on: true }).catch(() => {});
    for (const id of others) this.call(id);
    this.busy = false;
  }

  leave(tell = true) {
    for (const id of [...this.peers.keys()]) this.drop(id);
    this.stops.get(this.me)?.();
    this.stops.delete(this.me);
    this.stream?.getTracks().forEach((t) => t.stop());
    this.stream = null;
    this.ctx?.close().catch(() => {});
    this.ctx = null;
    if (this.on && tell) post(`/api/rooms/${this.room}/voice`, { on: false }).catch(() => {});
    this.on = false;
    this.speaking = {};
  }

  toggleMute() {
    this.muted = !this.muted;
    this.stream?.getAudioTracks().forEach((t) => (t.enabled = !this.muted));
  }

  /** Кто в чате - из комнаты; ушедших отключаем. */
  setIds(ids: number[]) {
    this.ids = ids;
    for (const id of [...this.peers.keys()]) if (!ids.includes(id)) this.drop(id);
  }

  async signal(from: number, data: Signal) {
    if (!this.on || from === this.me) return;
    const pc = this.pc(from);
    try {
      if (data.type === 'offer') {
        if (pc.signalingState !== 'stable') {
          // Позвонили друг другу разом: уступает тот, у кого id больше.
          if (this.me < from) return;
          await pc.setLocalDescription({ type: 'rollback' });
        }
        await pc.setRemoteDescription(data.sdp);
        await pc.setLocalDescription(await pc.createAnswer());
        if (pc.localDescription) this.send(from, { type: 'answer', sdp: pc.localDescription.toJSON() });
      } else if (data.type === 'answer') {
        if (pc.signalingState === 'have-local-offer') await pc.setRemoteDescription(data.sdp);
      } else if (data.type === 'ice') {
        await pc.addIceCandidate(data.candidate).catch(() => {});
      }
    } catch {
      /* связь с этим человеком пересоберётся при следующем входе */
    }
  }

  private pc(id: number): RTCPeerConnection {
    const have = this.peers.get(id);
    if (have) return have;
    const pc = new RTCPeerConnection({ iceServers: ICE });
    this.peers.set(id, pc);
    this.stream?.getTracks().forEach((t) => pc.addTrack(t, this.stream!));
    pc.onicecandidate = (e) => e.candidate && this.send(id, { type: 'ice', candidate: e.candidate.toJSON() });
    pc.ontrack = (e) => {
      const s = e.streams[0] ?? new MediaStream([e.track]);
      let a = this.audios.get(id);
      if (!a) {
        a = new Audio();
        a.autoplay = true;
        this.audios.set(id, a);
      }
      a.srcObject = s;
      a.play().catch(() => {});
      this.meter(id, s);
    };
    pc.onconnectionstatechange = () => {
      if (pc.connectionState === 'failed') this.drop(id);
    };
    return pc;
  }

  private async call(id: number) {
    const pc = this.pc(id);
    try {
      await pc.setLocalDescription(await pc.createOffer());
      if (pc.localDescription) this.send(id, { type: 'offer', sdp: pc.localDescription.toJSON() });
    } catch {
      this.drop(id);
    }
  }

  private drop(id: number) {
    this.peers.get(id)?.close();
    this.peers.delete(id);
    const a = this.audios.get(id);
    if (a) a.srcObject = null;
    this.audios.delete(id);
    this.stops.get(id)?.();
    this.stops.delete(id);
    const { [id]: _, ...rest } = this.speaking;
    this.speaking = rest;
  }

  /** Говорит ли человек - громкость его звука раз в 150 мс. */
  private meter(id: number, s: MediaStream) {
    this.stops.get(id)?.();
    try {
      this.ctx ??= new AudioContext();
      const src = this.ctx.createMediaStreamSource(s);
      const an = this.ctx.createAnalyser();
      an.fftSize = 256;
      src.connect(an);
      const buf = new Uint8Array(an.frequencyBinCount);
      const t = setInterval(() => {
        an.getByteFrequencyData(buf);
        const level = buf.reduce((a, b) => a + b, 0) / buf.length;
        const on = level > 18 && !(id === this.me && this.muted);
        if (!!this.speaking[id] !== on) this.speaking = { ...this.speaking, [id]: on };
      }, 150);
      this.stops.set(id, () => {
        clearInterval(t);
        src.disconnect();
      });
    } catch {
      /* без индикатора - голос всё равно идёт */
    }
  }
}
