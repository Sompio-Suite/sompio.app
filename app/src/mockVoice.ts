// Voice in the browser preview: a pretend server with a few people, so the screens can be seen.
import type { VoiceChannel, VoiceClient, VoiceSaved } from "./api";

type Emit = (event: string, payload: unknown) => void;
let emit: Emit = () => {};
export function setVoiceEmit(e: Emit) { emit = e; }

let saved: VoiceSaved = {
  bookmarks: [{ label: "Night Owls", address: "voice.nightowls.example", name: "Sam", fingerprint: "5AC2:74D6:59A7:F3E0:1CCE:5274:AD6D:A52A", password: null, last_channel: null }],
  push_to_talk: "",
  threshold: 0.02,
  nickname: "Sam",
};
const channels: VoiceChannel[] = [
  { id: 1, parent: null, name: "Lobby", topic: "Say hi", has_password: false, permanent: true, max_users: 0, order: 0 },
  { id: 2, parent: null, name: "Raid night", topic: "Wednesdays 20:00, Molten Core", has_password: false, permanent: true, max_users: 0, order: 1 },
  { id: 5, parent: 2, name: "Healers", topic: "", has_password: true, permanent: true, max_users: 5, order: 0 },
  { id: 3, parent: null, name: "Chill", topic: "", has_password: false, permanent: true, max_users: 0, order: 2 },
  { id: 4, parent: null, name: "AFK", topic: "", has_password: false, permanent: true, max_users: 0, order: 3 },
];
let clients: VoiceClient[] = [
  { id: 2, name: "Mara", channel: 2, role: "admin", muted: false, deafened: false, away: null, identity: "9f3a11c2" },
  { id: 3, name: "Sam", channel: 2, role: "moderator", muted: false, deafened: false, away: null, identity: "77b0e4d1" },
  { id: 4, name: "Noor", channel: 2, role: "member", muted: true, deafened: false, away: null, identity: "0c55af90" },
  { id: 5, name: "Jules", channel: 4, role: "member", muted: true, deafened: true, away: "brb", identity: "e1d2c3b4" },
];
let talker: number | null = null;

export const voiceHandlers: Record<string, (a: any) => unknown> = {
  voice_settings: () => saved,
  voice_save_settings: ({ settings }) => { saved = settings; },
  voice_connect: ({ name, trusted }) => {
    if (!trusted) return { kind: "new_server", fingerprint: "5AC2:74D6:59A7:F3E0:1CCE:5274:AD6D:A52A", name: "Night Owls" };
    const me: VoiceClient = { id: 1, name, channel: 2, role: "member", muted: false, deafened: false, away: null, identity: "a1b2c3d4" };
    clients = [me, ...clients.filter((c) => c.id !== 1)];
    // Someone talks now and then
    if (talker === null) talker = window.setInterval(() => {
      for (const id of [2, 3]) if (Math.random() < 0.5) for (let i = 0; i < 6; i++) setTimeout(() => emit("voice-audio", { from: id, seq: i, data: [1, 2, 3] }), i * 20);
    }, 700);
    setTimeout(() => emit("voice-event", { type: "chat", from: 2, from_name: "Mara", target: { to: "channel" }, text: "Pull in 5, everyone on voice?", at_ms: Date.now() }), 400);
    setTimeout(() => emit("voice-event", { type: "chat", from: 3, from_name: "Sam", target: { to: "channel" }, text: "Ready. Healers check the sub-channel.", at_ms: Date.now() }), 900);
    return { kind: "connected", fingerprint: trusted, welcome: { type: "welcome", you: 1, role: "member", server: { name: "Night Owls", welcome: "Welcome to Night Owls! Raids on Wednesday, be nice.", max_clients: 64, default_channel: 1 }, channels, clients } };
  },
  voice_disconnect: () => { if (talker !== null) window.clearInterval(talker); talker = null; },
  voice_send: ({ msg }) => {
    if (msg.type === "chat") setTimeout(() => emit("voice-event", { type: "chat", from: 1, from_name: clients.find((c) => c.id === 1)?.name ?? "You", target: msg.target, text: msg.text, at_ms: Date.now() }), 50);
    if (msg.type === "join") { const me = { ...clients.find((c) => c.id === 1)!, channel: msg.channel }; clients = clients.map((c) => (c.id === 1 ? me : c)); setTimeout(() => emit("voice-event", { type: "client_updated", client: me }), 50); }
    if (msg.type === "set_state") { const me = { ...clients.find((c) => c.id === 1)!, muted: msg.muted || msg.deafened, deafened: msg.deafened }; clients = clients.map((c) => (c.id === 1 ? me : c)); setTimeout(() => emit("voice-event", { type: "client_updated", client: me }), 50); }
    if (msg.type === "use_privilege_key") setTimeout(() => emit("voice-event", { type: "error", message: "That key isn't valid on this server, or was used already." }), 50);
  },
  voice_ping: () => 24,
  voice_host_start: ({ port, name }) => ({ port, fingerprint: "1F0B:9C2E:44A1:7D3E:BB10:6E02:C9A4:3F77", admin_key: "HE39-U4M6-6CKN-7W9B-QYTP", name, lan: "192.168.1.23", public: "81.204.37.12", reach: "open", reach_note: "Your router opened the port. Friends anywhere can join with this address." }),
  voice_host_open_port: () => ({}),
  voice_host_stop: () => undefined,
  voice_host_status: () => null,
};
