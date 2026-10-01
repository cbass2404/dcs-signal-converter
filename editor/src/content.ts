// Reading and writing the pieces of a display field.
//
// A field is a chain of spans, but a chain of one is stored flat, with that
// span's source and styling written beside the cells. That is the shape every
// profile written before chains is in, and the shape they have to stay in: an
// update never rewrites a row the user has changed, so a field that came back
// from a save as a `content` array where a `source` used to be would turn
// every row into a row the user owns and freeze it against every later fix.
//
// The window works in spans and nothing else. These two functions are the
// whole of the conversion, and it only runs one way: `contentOf` reads a field
// however it happens to be written, and `setContent` always writes the array.
// The backend collapses a chain of one back to the flat shape on the way to
// disk, which is where the guarantee actually lives and where it is tested.

import type { Readout, Span, SpanPatch } from "./types";

/** The keys a flat field keeps its one span's settings under. */
const FLAT = [
  "text",
  "source",
  "gap",
  "reads",
  "conversions",
  "decimals",
  "digits",
  "round",
  "wrap",
  "abs",
  "value_aliases",
  "aliases",
  "format",
  "colour",
  "small",
  "inverse",
  "colours",
  "replace",
] as const;

/** The pieces of a field, however the profile happened to write it. */
export function contentOf(readout: Readout): Span[] {
  if (readout.content && readout.content.length > 0) return readout.content;
  const one: Span = {};
  if (readout.text) one.text = readout.text;
  if (readout.source) one.source = readout.source;
  if (readout.gap) one.gap = readout.gap;
  if (readout.reads) one.reads = readout.reads;
  if (readout.conversions) one.conversions = readout.conversions;
  if (readout.decimals) one.decimals = readout.decimals;
  if (readout.digits) one.digits = readout.digits;
  if (readout.round) one.round = readout.round;
  if (readout.wrap) one.wrap = readout.wrap;
  if (readout.abs) one.abs = readout.abs;
  if (readout.value_aliases) one.value_aliases = readout.value_aliases;
  if (readout.aliases) one.aliases = readout.aliases;
  if (readout.format !== undefined) one.format = readout.format;
  if (readout.colour) one.colour = readout.colour;
  if (readout.small) one.small = readout.small;
  if (readout.inverse) one.inverse = readout.inverse;
  if (readout.colours) one.colours = readout.colours;
  if (readout.replace) one.replace = readout.replace;
  return [one];
}

/**
 * Put the pieces back on the field, and clear the flat shape.
 *
 * The flat keys go because leaving them would mean a field described twice,
 * and whichever the backend preferred the other would be a stale copy waiting
 * to be believed.
 */
export function setContent(readout: Readout, spans: Span[]): void {
  readout.content = spans;
  for (const key of FLAT) delete (readout as unknown as Record<string, unknown>)[key];
  // `source` is not optional on the type, because every field had one before
  // chains existed and a good deal of the window still reads it.
  readout.source = "";
}

/**
 * A whole-field divider, from a file written before a rule was a piece, made
 * into a field of one rule piece, which draws the same line.
 *
 * Run on every field as it reaches the window, so nothing past this point has
 * a divider to draw or edit. Only the window's copy changes: the file keeps
 * its divider, which the engine still draws, until the page is next saved.
 */
export function ruleFromDivider(readout: Readout): void {
  if (!readout.divider) return;
  const rule: Span = { gap: true, rule: true };
  if (readout.colour) rule.colour = readout.colour;
  if (readout.small) rule.small = readout.small;
  if (readout.label) rule.label = readout.label;
  if (readout.label_colour) rule.label_colour = readout.label_colour;
  delete readout.divider;
  delete readout.label;
  delete readout.label_colour;
  setContent(readout, [rule]);
}

/** A new empty piece of the kind asked for. */
export function newSpan(kind: SpanKind): Span {
  if (kind === "rule") return { gap: true, rule: true };
  if (kind === "gap") return { gap: true };
  if (kind === "switch") return { switch: "", source: "", cases: {} };
  if (kind === "stored") return { signal: "" };
  return kind === "text" ? { text: "" } : { source: "" };
}

export type SpanKind = "text" | "signal" | "stored" | "gap" | "rule" | "switch";

/**
 * Which of the four a piece is.
 *
 * The key being present is what says so, not what is in it. A piece the user
 * has just added reads no signal yet, and asking whether `source` held
 * anything made it read back as text: the menu said text and a text box was
 * what they got. Whether it has been filled in is a separate question, and the
 * profile check is what asks it.
 *
 * Nothing writes an empty `source` onto a piece of text: the backend drops the
 * key when it is empty, `contentOf` only sets it from a field that has one,
 * and every piece the window builds is built through `newSpan`.
 *
 * A rule is a gap that draws dashes instead of blanks, so it is one kind of
 * piece in the menu and one key on top of a gap in the file. Asked in that
 * order, because everything true of a gap is true of it.
 *
 * A switch is asked about first, because the `source` it carries is one its
 * cases share rather than one it reads.
 */
export function kindOf(span: Span): SpanKind {
  if ("switch" in span) return "switch";
  if (span.rule) return "rule";
  if (span.gap) return "gap";
  if ("signal" in span) return "stored";
  return "source" in span ? "signal" : "text";
}

/**
 * Whether this piece draws characters the user typed rather than a reading.
 *
 * A gap is not literal in this sense: it draws nothing at all, and every
 * caller that asks this is about to do something with characters.
 */
export function isLiteral(span: Span): boolean {
  return kindOf(span) === "text";
}

// --- switches ---------------------------------------------------------------
//
// A switch is one piece whose reading another signal decides: the Huey's ADF
// needle runs 0 to 65535 whichever band is selected, and the band switch says
// whether that is 190 to 400 kHz or 850 to 1750. What the switch carries
// besides its selector is shared by its cases, and a case piece writes only
// what differs. These mirror `SpanPatch::resolve` in the config crate, which
// is what the panel draws by; the window needs the same answer per keystroke.

/** What a piece decides for itself and never takes from its switch. */
const OWN = ["text", "signal", "gap", "rule", "label", "label_colour", "width", "align"] as const;

/** What a reading takes from its switch where it leaves it unset. */
const SHAPING = [
  "source",
  "reads",
  "conversions",
  "decimals",
  "digits",
  "round",
  "wrap",
  "value_aliases",
  "abs",
  "aliases",
  "format",
  "colours",
] as const;

/** What every piece takes from its switch, typed characters and rules too. */
const STYLING = ["colour", "small", "inverse"] as const;

/** Every option a case can leave to its switch. */
const SHARED = [...SHAPING, ...STYLING, "replace"] as const;

type SharedKey = (typeof SHARED)[number];

/**
 * Options whose control has no empty state, so taking one away is a choice:
 * an unticked box, a menu set back to its first entry, decimals typed as 0.
 * The rest, a wrap or a set of words, are emptied to hand them back to the
 * switch, and are turned off for one case by the off toggle instead.
 */
const CHOSEN_BY_ABSENCE = new Set<string>([
  "abs",
  "small",
  "inverse",
  "round",
  "decimals",
  "format",
  "source",
]);

/** The options a switch shares that a case can turn off with its toggle. */
export const SWITCHABLE_OFF = ["wrap", "digits", "value_aliases"] as const;

/** What a case piece can be. A stored signal is drawn, not shaped, so it takes styling only. */
type PieceKind = "gap" | "text" | "signal" | "stored";

/** The kind a case piece is, read the way `kindOf` reads a piece. */
function patchKind(patch: SpanPatch): PieceKind {
  if (patch.gap) return "gap";
  if ("text" in patch) return "text";
  return "signal" in patch ? "stored" : "signal";
}

/** Whether a piece of this kind takes `key` from its switch. */
function takes(kind: PieceKind, key: SharedKey): boolean {
  if ((STYLING as readonly string[]).includes(key)) return true;
  if (key === "replace") return kind !== "gap";
  return kind === "signal";
}

function copy<T>(value: T): T {
  return value === undefined ? value : (JSON.parse(JSON.stringify(value)) as T);
}

function same(a: unknown, b: unknown): boolean {
  const sorted = (v: unknown): unknown => {
    if (Array.isArray(v)) return v.map(sorted);
    if (v === null || typeof v !== "object") return v;
    const out: Record<string, unknown> = {};
    for (const k of Object.keys(v as object).sort()) {
      out[k] = sorted((v as Record<string, unknown>)[k]);
    }
    return out;
  };
  return JSON.stringify(sorted(a)) === JSON.stringify(sorted(b));
}

/**
 * The piece a case draws: what it wrote, with what it left unset taken from
 * the switch. `reads` and `conversions` are one choice made two ways, so a
 * case that makes it either way, or clears it, takes neither from the switch.
 */
export function resolvePiece(patch: SpanPatch, shared: Span): Span {
  const kind = patchKind(patch);
  const converts =
    kind === "signal" && patch.reads === undefined && patch.conversions === undefined;
  const out: Record<string, unknown> = {};
  for (const key of OWN) {
    const v = patch[key];
    if (v !== undefined && v !== null) out[key] = v;
  }
  for (const key of SHARED) {
    const v = patch[key];
    if (v === null) continue;
    if (v !== undefined) {
      out[key] = copy(v);
      continue;
    }
    const inherit =
      key === "reads" || key === "conversions" ? converts : takes(kind, key as SharedKey);
    if (inherit && shared[key] !== undefined) out[key] = copy(shared[key]);
  }
  // A reading with nothing chosen is still a reading, so the menu says so.
  if (kind === "signal" && out.source === undefined) out.source = "";
  return out as Span;
}

/**
 * What a case piece has to write for the window's piece `view` to come out
 * of `resolvePiece`: only what differs from the switch.
 *
 * A value equal to the switch's is left to the switch, so changing the shared
 * one later changes this case too. A value taken away goes back to the switch
 * where its box can be emptied, and is written `null` where taking it away is
 * a choice. An option already turned off stays off until something is typed
 * into it.
 */
export function patchOf(view: Span, shared: Span, was: SpanPatch): SpanPatch {
  const kind: PieceKind = view.gap
    ? "gap"
    : "text" in view
      ? "text"
      : "signal" in view
        ? "stored"
        : "signal";
  const out: SpanPatch = {};
  const write = out as Record<string, unknown>;
  for (const key of OWN) {
    const v = view[key];
    if (key === "text" ? "text" in view : v !== undefined) write[key] = v;
  }
  // What this kind of piece would take if it set nothing.
  const base = resolvePiece(
    kind === "gap"
      ? { gap: true }
      : kind === "text"
        ? { text: "" }
        : kind === "stored"
          ? { signal: view.signal ?? "" }
          : {},
    shared,
  );
  for (const key of SHARED) {
    if (!takes(kind, key)) {
      if (view[key] !== undefined) write[key] = view[key];
      continue;
    }
    const v = view[key];
    const b = base[key];
    if (v !== undefined) {
      if (!same(v, b)) write[key] = v;
      continue;
    }
    if (b === undefined) continue;
    if (was[key] === null || CHOSEN_BY_ABSENCE.has(key)) write[key] = null;
  }
  // A reading drawn as sent where the switch converts: neither half of the
  // conversion is inherited, which one null says.
  if (
    kind === "signal" &&
    view.reads === undefined &&
    view.conversions === undefined &&
    (base.reads !== undefined || base.conversions !== undefined)
  ) {
    write.reads = null;
    delete write.conversions;
  }
  // A source left as the switch's is not written. An empty one never is.
  if (write.source === "") delete write.source;
  return out;
}

/**
 * Whether a case piece takes `key` from its switch rather than setting it,
 * which is what greys its control out.
 */
export function inherits(view: Span, patch: SpanPatch, shared: Span, key: string): boolean {
  const kind = view.gap ? "gap" : "text" in view ? "text" : "signal";
  if (key === "reads" || key === "conversions") {
    return (
      kind === "signal" &&
      patch.reads === undefined &&
      patch.conversions === undefined &&
      (shared.reads !== undefined || shared.conversions !== undefined)
    );
  }
  if (!(SHARED as readonly string[]).includes(key)) return false;
  const k = key as SharedKey;
  return patch[k] === undefined && takes(kind, k) && shared[k] !== undefined;
}

/** The positions a case key claims, asked one at a time. `else` claims none itself. */
export function keyClaims(key: string, position: number): boolean {
  const s = key.trim();
  if (s === "else") return false;
  const split = s.includes("..") ? ".." : s.includes(" to ") ? " to " : "";
  if (split) {
    const [lo, hi] = s.split(split).map((part) => Number(part.trim()));
    return lo !== undefined && hi !== undefined && position >= lo && position <= hi;
  }
  return s.split(",").some((part) => Number(part.trim()) === position);
}

/** A case's pieces as written, one object or a chain. */
export function patchesOf(written: SpanPatch | SpanPatch[]): SpanPatch[] {
  return Array.isArray(written) ? written : [written];
}

/** The lowest reading a case key names, for putting cases in matching order. */
function keyStart(key: string): number {
  if (key.trim() === "else") return Number.POSITIVE_INFINITY;
  const first = key.split(/\.\.|,| to /)[0] ?? "";
  const n = Number(first.trim());
  return Number.isFinite(n) ? n : Number.POSITIVE_INFINITY;
}

/**
 * A switch's case keys in the order they are matched: by where each starts,
 * `else` last, the order the daemon holds them in.
 */
export function caseKeys(span: Span): string[] {
  return Object.keys(span.cases ?? {}).sort((a, b) => {
    const d = keyStart(a) - keyStart(b);
    if (d !== 0 && Number.isFinite(d)) return d;
    if (a.trim() === "else") return 1;
    if (b.trim() === "else") return -1;
    return a.localeCompare(b);
  });
}

/** The pieces one case of a switch draws. */
export function casePieces(span: Span, key: string): Span[] {
  const written = span.cases?.[key];
  if (!written) return [];
  return patchesOf(written).map((p) => resolvePiece(p, span));
}

/**
 * A chain with every switch replaced by the pieces of one of its cases, which
 * is what the panel lays out in a frame. `pick` says which case, and a switch
 * with none to give draws nothing.
 */
export function expandSwitches(spans: Span[], pick: (span: Span) => string | undefined): Span[] {
  return spans.flatMap((s) => {
    if (kindOf(s) !== "switch") return [s];
    const key = pick(s);
    return key === undefined ? [] : casePieces(s, key);
  });
}
