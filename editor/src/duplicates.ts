// Readings shaped the same way more than once: a shared signal waiting to be
// made.
//
// Two fields drawing the radar altimeter each say how, in full, and a fix to
// one is a fix the other misses. Found here across the module's saved pages,
// so the Shared Signals section can offer to make the signal once and point
// every one of them at it. Each keeps how it draws, its words, colours and
// box; only the shaping moves.
//
// Only readings that shape a number count, the way the converter's own
// `shapes_a_number` decides: a knob drawn as words beside the same knob drawn
// as a number is reuse, not a duplicate. Lamps never count, nor do the pieces
// inside a switch's cases, which take their shaping from the switch, nor a
// reading that colours a stretch of its conversion, which a shared signal
// refuses.

import { contentOf, setContent } from "./content";
import type { Page, Span, Term } from "./types";

/** The keys that shape a number, which move to the shared signal. */
const SHAPING = ["reads", "conversions", "decimals", "digits", "round", "wrap", "abs"] as const;

/** One reading's place: a page, its field, and the piece in that field. */
export interface Use {
  page: Page;
  field: number;
  piece: number;
}

/** One DCS-BIOS signal shaped the same way in two or more readings. */
export interface Duplicate {
  /** The shaping, written the same way every time, which tells one group from another. */
  key: string;
  /** The part the shared signal is made of. */
  term: Term;
  uses: Use[];
}

/** What a reading's shaping amounts to as a part, or null for one that shapes nothing. */
function shapingOf(span: Span): Term | null {
  if (!span.source || span.signal || span.switch || span.switch_signal) return null;
  // A shared signal's conversion carries no colour or size, since how it
  // draws is the piece's, so a reading styled that way cannot move as it is.
  if (span.conversions?.some((c) => c.colour !== undefined || c.small)) return null;
  const term: Term = { source: span.source };
  let shaped = false;
  for (const key of SHAPING) {
    const value = span[key];
    const set =
      value !== undefined &&
      value !== null &&
      value !== 0 &&
      value !== false &&
      !(Array.isArray(value) && value.length === 0);
    if (set) {
      (term as Record<string, unknown>)[key] = value;
      shaped = true;
    }
  }
  return shaped ? term : null;
}

/** Every signal shaped the same way in two or more readings on `pages`. */
export function duplicates(pages: Page[]): Duplicate[] {
  const found = new Map<string, Duplicate>();
  for (const page of pages) {
    page.fields.forEach((readout, field) => {
      contentOf(readout).forEach((span, piece) => {
        const term = shapingOf(span);
        if (!term) return;
        const key = JSON.stringify(term);
        const group = found.get(key) ?? { key, term, uses: [] };
        group.uses.push({ page, field, piece });
        found.set(key, group);
      });
    });
  }
  return [...found.values()].filter((d) => d.uses.length > 1);
}

/** The names of the pages a group's readings are on, once each. */
export function pagesOf(d: Duplicate): string[] {
  return [...new Set(d.uses.map((u) => u.page.name))];
}

/** Where one reading sits on its page: its cells, and which piece where there are several. */
export function placeOf(use: Use): string {
  const field = use.page.fields[use.field];
  const pieces = field ? contentOf(field).length : 1;
  const piece = pieces > 1 ? `, piece ${use.piece + 1} of ${pieces}` : "";
  return `cells ${field?.cells ?? "?"}${piece}`;
}

/** The pages a group's readings are on, once each, as they stood when it was found. */
export function pagesIn(d: Duplicate): Page[] {
  return [...new Map(d.uses.map((u) => [u.page.id, u.page])).values()];
}

/**
 * Point every reading in `d` on `page` at the shared signal `id`, in place.
 * `page` is the page the group was found on or a copy of it, so the fields
 * sit where they did. Each piece keeps everything that says how it draws.
 */
export function repoint(d: Duplicate, id: string, page: Page): void {
  for (const use of d.uses) {
    if (use.page.id !== page.id) continue;
    const readout = page.fields[use.field];
    if (!readout) continue;
    const spans = contentOf(readout);
    const span = spans[use.piece];
    if (!span) continue;
    delete span.source;
    for (const key of SHAPING) delete span[key];
    span.signal = id;
    // Always written as a chain: a flat field has no key for a shared signal,
    // and the backend writes a chain of one back flat where it can.
    setContent(readout, spans);
  }
}

/** Groups put away with their × on `module`, kept for this viewer only. */
export function dismissed(module: string): Set<string> {
  try {
    const raw = localStorage.getItem(`dsc-duplicates-dismissed:${module}`);
    return new Set(raw ? (JSON.parse(raw) as string[]) : []);
  } catch {
    return new Set();
  }
}

/** Put these groups away on `module`, so their banner does not come back. */
export function dismiss(module: string, keys: string[]): void {
  try {
    const all = dismissed(module);
    for (const k of keys) all.add(k);
    localStorage.setItem(`dsc-duplicates-dismissed:${module}`, JSON.stringify([...all]));
  } catch {
    // Storage refused: the banner comes back next time, which is harmless.
  }
}
