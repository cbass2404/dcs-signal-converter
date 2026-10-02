// The open module's stored signals, for any piece that draws one.
//
// Held here rather than handed down through every field editor, the way the
// gauge tables are: one module is open at a time, and the page book keeps
// this up to date whenever the library tells it what the module holds.

import type { StoredSignal, Term } from "./types";

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

/**
 * What may be typed into a shared result's symbol part: what a number is
 * written with, so the result stays a number its bands can match. Words are
 * the field's that draws it. The config crate's `is_number_symbol`.
 */
export const NUMBER_SYMBOLS = /^[0-9.+-]*$/;

/** Whether a shared result's part is symbols typed in rather than a reading. */
export function isSymbolPart(t: Term): boolean {
  return "text" in t;
}

/** A shared result's part as a summary line names it. */
export function partName(t: Term): string {
  if (isSymbolPart(t)) return t.text ? JSON.stringify(t.text) : "a symbol part left empty";
  return t.source || "a part with no signal yet";
}

/** One stored signal by id, where the module has it. */
export function storedSignal(id: string): StoredSignal | undefined {
  return current.find((s) => s.id === id);
}
