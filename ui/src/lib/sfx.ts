// The game's own menu sounds, played from the user's ROM through the asset
// bridge (sequence names from the sound archive; inventory section 12).
import { asset } from "./backend";

const NAMES = {
  select: "SEQ_SE_SELECT1",
  decide: "SEQ_SE_DECIDE1",
  cancel: "SEQ_SE_CANCEL1",
  save: "SEQ_SE_SAVE",
  open: "SEQ_SE_OPEN1",
  close: "SEQ_SE_CLOSE1",
} as const;

export type Sfx = keyof typeof NAMES;

const KEY = "kenon.sfx";
let enabled = (() => {
  try {
    return localStorage.getItem(KEY) !== "off";
  } catch {
    return true;
  }
})();
let ready = false;
const cache = new Map<Sfx, HTMLAudioElement>();

/// Called once a game is open, so the clips can load.
export function enableSfx(gameOpen: boolean) {
  ready = gameOpen;
  cache.clear();
}

export function sfxOn(): boolean {
  return enabled;
}

export function setSfx(on: boolean) {
  enabled = on;
  try {
    localStorage.setItem(KEY, on ? "on" : "off");
  } catch {
    // Not saved; the setting lasts for this session.
  }
}

export function play(name: Sfx) {
  if (!enabled || !ready) return;
  let audio = cache.get(name);
  if (!audio) {
    audio = new Audio(asset(`sound/${NAMES[name]}?seconds=3`));
    audio.volume = 0.5;
    cache.set(name, audio);
  }
  audio.currentTime = 0;
  audio.play().catch(() => {});
}
