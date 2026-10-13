import { readonly, ref } from "vue";
import { createSharedComposable } from "@vueuse/core";
import { readPref, writePref } from "@workbench/utils";

// Sampled switch packs from kbsim and Mechvibes (MIT, see public/sounds/*/LICENSE), taken from tabsh:
// five press variants for ordinary keys, picked at random, and their own press/release for the
// big keys. A null release means press-only samples.

export const TYPING_SOUND_KEY = "workbench.typingSound";

type Special = "space" | "enter" | "back";
export const SOUND_PACKS = {
  "holy-panda": { label: "Holy Panda", release: "release_key", special: ["space", "enter", "back"] },
  "gateron-black-ink": { label: "Gateron Black Ink", release: "release_key", special: ["space", "enter", "back"] },
  "mx-black-pbt": { label: "Cherry MX Black", release: null, special: ["space", "enter", "back"] },
  "mx-blue": { label: "Cherry MX Blue", release: "release", special: [] },
} satisfies Record<string, { label: string; release: string | null; special: Special[] }>;
export type SoundPack = keyof typeof SOUND_PACKS;
export type TypingSoundSetting = SoundPack | "off";

const SPECIAL_KEYS: Record<string, Special> = { " ": "space", Enter: "enter", Backspace: "back", Delete: "back" };

interface Sample { press: AudioBuffer; release: AudioBuffer | null }
interface Pack { keys: Sample[]; space?: Sample; enter?: Sample; back?: Sample }

let audio: AudioContext | null = null;
let gain: GainNode | null = null;
const loads: Partial<Record<SoundPack, Promise<Pack>>> = {};
const loaded: Partial<Record<SoundPack, Pack>> = {};

function context(): AudioContext | null {
  if (typeof AudioContext === "undefined") return null;
  return (audio ??= new AudioContext());
}

/** Fetch and decode a pack; decoding needs a context but not a user gesture. */
export function loadPack(id: SoundPack): Promise<Pack> {
  const existing = loads[id];
  if (existing) return existing;
  const ctx = context();
  if (!ctx) return Promise.reject(new Error("Web Audio is unavailable"));
  const { release, special } = SOUND_PACKS[id] as { release: string | null; special: Special[] };
  const buffer = (name: string) =>
    fetch(`/sounds/${id}/${name}.mp3`)
      .then(res => { if (!res.ok) throw new Error(res.statusText); return res.arrayBuffer(); })
      .then(data => ctx.decodeAudioData(data));
  const keyRelease = release ? buffer(release) : null;
  const sample = async (press: string, rel: Promise<AudioBuffer> | null): Promise<Sample> => ({ press: await buffer(press), release: await rel });
  const load = Promise.all([
    Promise.all([1, 2, 3, 4, 5].map(n => sample(`press_key${n}`, keyRelease))),
    ...special.map(s => sample(`press_${s}`, release ? buffer(`release_${s}`) : null)),
  ])
    .then(([keys, ...rest]) => (loaded[id] = { keys: keys as Sample[], ...Object.fromEntries(special.map((s, i) => [s, rest[i]])) }))
    .catch(err => { delete loads[id]; throw err; });
  loads[id] = load;
  return load;
}

export function playSample(buf: AudioBuffer) {
  if (!audio) return;
  if (audio.state !== "running") void audio.resume().catch(() => {});
  if (!gain) {
    gain = audio.createGain();
    gain.gain.value = 0.6;
    gain.connect(audio.destination);
  }
  const src = audio.createBufferSource();
  src.buffer = buf;
  src.playbackRate.value = 0.95 + Math.random() * 0.1;
  src.connect(gain);
  src.start();
}

/** Whether a keydown types (or deletes) rather than being a shortcut, navigation or IME composition. */
export function typesText(event: KeyboardEvent) {
  if (event.metaKey || event.ctrlKey || event.altKey || event.isComposing || event.repeat) return false;
  return event.key.length === 1 || event.key in SPECIAL_KEYS;
}

/** Play the press for a key; returns the release to play on keyup, if the pack has one. Silent while loading. */
export function keySound(id: SoundPack, key: string): AudioBuffer | null {
  const pack = loaded[id];
  if (!pack) { loadPack(id).catch(() => {}); return null; }
  const special = SPECIAL_KEYS[key];
  const sample = (special && pack[special]) || pack.keys[Math.floor(Math.random() * pack.keys.length)]!;
  playSample(sample.press);
  return sample.release;
}

function parse(value: string | null): TypingSoundSetting {
  if (value === "off") return "off";
  return value && value in SOUND_PACKS ? (value as SoundPack) : "holy-panda";
}

/** The rich editor's typing sound: a switch pack, or off. */
export const useTypingSound = createSharedComposable(() => {
  const setting = ref<TypingSoundSetting>(parse(readPref(TYPING_SOUND_KEY)));
  function set(next: TypingSoundSetting) {
    setting.value = next;
    writePref(TYPING_SOUND_KEY, next === "holy-panda" ? null : next);
    // Play one key so the choice is heard straight away.
    if (next !== "off") loadPack(next).then(() => {
      const release = keySound(next, "a");
      if (release) setTimeout(() => playSample(release), 90);
    }).catch(() => {});
  }
  return { setting: readonly(setting), set };
});
