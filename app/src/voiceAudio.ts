// Voice audio in the window: microphone (with the browser engine's echo cancellation, noise
// suppression and gain control), Opus encoding with WebCodecs, and playback with a small
// jitter buffer per speaker. Packets go to the Rust side, which sends them to the server.

import { sendVoiceFrame } from "./api";

const RATE = 48000;
const FRAME = 960;
/** How far ahead playback runs, to absorb network jitter */
const BUFFER_S = 0.06;
/** Voice activation keeps sending this long after the level drops */
const HANGOVER_MS = 350;

export function voiceSupported(): boolean {
  return typeof AudioEncoder !== "undefined" && typeof AudioDecoder !== "undefined" && !!navigator.mediaDevices?.getUserMedia;
}

export interface EngineOptions {
  deviceId?: string;
  threshold: number;
  onLevel?: (level: number, sending: boolean) => void;
  onSpeaking?: (who: number, speaking: boolean) => void;
  onError?: (message: string) => void;
}

interface Speaker {
  decoder: AudioDecoder;
  next: number;
  last: number;
  gain: GainNode;
}

export class VoiceEngine {
  private ctx: AudioContext | null = null;
  private stream: MediaStream | null = null;
  private node: AudioWorkletNode | null = null;
  private encoder: AudioEncoder | null = null;
  private seq = 0;
  private ts = 0;
  private lastLoud = 0;
  private speakers = new Map<number, Speaker>();
  private sweep: number | null = null;
  private wasSending = false;
  muted = false;
  deafened = false;
  /** null: voice activation; true/false: push-to-talk key state */
  ptt: boolean | null = null;
  volumes = new Map<number, number>();

  constructor(private opts: EngineOptions) {}

  set threshold(t: number) {
    this.opts.threshold = t;
  }

  async start() {
    if (!voiceSupported()) throw new Error("Voice needs a newer version of Microsoft Edge WebView2. Run Windows Update, then try again.");
    this.ctx = new AudioContext({ sampleRate: RATE, latencyHint: "interactive" });
    await this.ctx.audioWorklet.addModule("/voice-capture.js");
    this.stream = await navigator.mediaDevices.getUserMedia({
      audio: {
        deviceId: this.opts.deviceId ? { exact: this.opts.deviceId } : undefined,
        echoCancellation: true,
        noiseSuppression: true,
        autoGainControl: true,
        channelCount: 1,
        sampleRate: RATE,
      },
    });
    const src = this.ctx.createMediaStreamSource(this.stream);
    this.node = new AudioWorkletNode(this.ctx, "voice-capture");
    src.connect(this.node);
    this.encoder = new AudioEncoder({
      output: (chunk) => {
        const bytes = new Uint8Array(chunk.byteLength);
        chunk.copyTo(bytes);
        sendVoiceFrame(this.seq, bytes);
        this.seq = (this.seq + 1) & 0xffff;
      },
      error: (e) => this.opts.onError?.(`The voice encoder stopped: ${e.message}`),
    });
    this.encoder.configure({ codec: "opus", sampleRate: RATE, numberOfChannels: 1, bitrate: 48000 });
    this.node.port.onmessage = (e: MessageEvent<{ frame: Float32Array<ArrayBuffer>; level: number }>) => this.frame(e.data.frame, e.data.level);
    // Speaking lights go off shortly after someone's packets stop
    this.sweep = window.setInterval(() => {
      const now = performance.now();
      for (const [who, s] of this.speakers) {
        if (s.last && now - s.last > 250) {
          s.last = 0;
          this.opts.onSpeaking?.(who, false);
        }
      }
    }, 100);
  }

  private frame(frame: Float32Array<ArrayBuffer>, level: number) {
    const now = performance.now();
    if (level > this.opts.threshold) this.lastLoud = now;
    const open = this.ptt === null ? now - this.lastLoud < HANGOVER_MS : this.ptt;
    const sending = open && !this.muted && !this.deafened;
    if (sending !== this.wasSending) this.wasSending = sending;
    this.opts.onLevel?.(level, sending);
    if (!sending || !this.encoder || this.encoder.state !== "configured") return;
    const data = new AudioData({ format: "f32-planar", sampleRate: RATE, numberOfFrames: FRAME, numberOfChannels: 1, timestamp: this.ts, data: frame as Float32Array<ArrayBuffer> });
    this.ts += (FRAME * 1_000_000) / RATE;
    this.encoder.encode(data);
    data.close();
  }

  /** A packet from someone else in the channel. */
  receive(from: number, data: Uint8Array) {
    if (!this.ctx || this.deafened) return;
    let s = this.speakers.get(from);
    if (!s) {
      const gain = this.ctx.createGain();
      gain.connect(this.ctx.destination);
      const speaker: Speaker = {
        next: 0,
        last: 0,
        gain,
        decoder: new AudioDecoder({
          output: (audio) => this.play(speaker, audio),
          error: () => this.speakers.delete(from),
        }),
      };
      speaker.decoder.configure({ codec: "opus", sampleRate: RATE, numberOfChannels: 1 });
      this.speakers.set(from, speaker);
      s = speaker;
    }
    s.gain.gain.value = this.volumes.get(from) ?? 1;
    if (!s.last) this.opts.onSpeaking?.(from, true);
    s.last = performance.now();
    if (s.decoder.state === "configured") s.decoder.decode(new EncodedAudioChunk({ type: "key", timestamp: 0, data }));
  }

  private play(s: Speaker, audio: AudioData) {
    const ctx = this.ctx;
    if (!ctx) return audio.close();
    const buf = ctx.createBuffer(1, audio.numberOfFrames, audio.sampleRate);
    audio.copyTo(buf.getChannelData(0), { planeIndex: 0, format: "f32-planar" });
    audio.close();
    const node = ctx.createBufferSource();
    node.buffer = buf;
    node.connect(s.gain);
    const now = ctx.currentTime;
    // Start a little ahead; if we fell behind (a gap in speech), start fresh
    if (s.next < now || s.next > now + 0.5) s.next = now + BUFFER_S;
    node.start(s.next);
    s.next += buf.duration;
  }

  async stop() {
    if (this.sweep) window.clearInterval(this.sweep);
    this.node?.disconnect();
    this.stream?.getTracks().forEach((t) => t.stop());
    try { this.encoder?.close(); } catch { /* closed */ }
    for (const s of this.speakers.values()) {
      try { s.decoder.close(); } catch { /* closed */ }
    }
    this.speakers.clear();
    await this.ctx?.close().catch(() => {});
    this.ctx = null;
  }
}

export async function microphones(): Promise<MediaDeviceInfo[]> {
  try {
    return (await navigator.mediaDevices.enumerateDevices()).filter((d) => d.kind === "audioinput");
  } catch {
    return [];
  }
}
