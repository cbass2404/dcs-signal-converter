// The open module's stored signals, for any piece that draws one.
//
// Held here rather than handed down through every field editor, the way the
// gauge tables are: one module is open at a time, and the page book keeps
// this up to date whenever the library tells it what the module holds.

import type { StoredSignal } from "./types";

let current: StoredSignal[] = [];

/** The open module's stored signals, as last saved. */
export function storedSignals(): StoredSignal[] {
  return current;
}

/** Take in what the library says the module now holds. */
export function setStoredSignals(signals: StoredSignal[]): void {
  current = signals;
}

/** Whether a stored signal is lamp conditions rather than a number. */
export function isLampSignal(s: StoredSignal): boolean {
  return (s.conditions?.length ?? 0) > 0 || (s.any_of?.length ?? 0) > 0;
}

/** One stored signal by id, where the module has it. */
export function storedSignal(id: string): StoredSignal | undefined {
  return current.find((s) => s.id === id);
}
