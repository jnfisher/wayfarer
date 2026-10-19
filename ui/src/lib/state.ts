// What the brain has told the UI: one character snapshot per session and whether a brain is
// attached (docs/protocol.md, "Events, state and dev"). Plain functions on a plain object, so
// App.svelte can keep it in `$state` and the tests can drive it without Svelte.

import type { CharacterState } from "../proto";

export interface BrainView {
  /** The last snapshot per session, with every patch since applied. */
  characters: Record<string, CharacterState>;
  /** From `brain_state`; null until the gateway has said. */
  attached: boolean | null;
}

export function emptyBrainView(): BrainView {
  return { characters: {}, attached: null };
}

const isObject = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v);

/**
 * Apply a JSON merge patch (RFC 7386): objects recurse, `null` deletes, anything else
 * replaces. Changes `target` in place when it is an object (so a Svelte `$state` proxy updates
 * only the fields that changed) and returns the result.
 */
export function mergePatch(target: unknown, patch: unknown): unknown {
  if (!isObject(patch)) return patch;
  const out = isObject(target) ? target : {};
  for (const [key, value] of Object.entries(patch)) {
    if (value === null) delete out[key];
    else out[key] = mergePatch(out[key], value);
  }
  return out;
}

/** `character`: the whole snapshot replaces the last one. */
export function applyCharacter(view: BrainView, session: string, state: CharacterState): void {
  view.characters[session] = state;
}

/** `character_patch`: merged into the last snapshot (or an empty one). */
export function applyCharacterPatch(view: BrainView, session: string, patch: unknown): void {
  const current = view.characters[session];
  const next = mergePatch(current ?? {}, patch);
  if (next !== current) view.characters[session] = isObject(next) ? (next as CharacterState) : {};
}

export function applyBrainState(view: BrainView, attached: boolean): void {
  view.attached = attached;
}
