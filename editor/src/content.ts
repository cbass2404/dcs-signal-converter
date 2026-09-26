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

import type { Readout, Span } from "./types";

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
  return kind === "text" ? { text: "" } : { source: "" };
}

export type SpanKind = "text" | "signal" | "gap" | "rule";

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
 */
export function kindOf(span: Span): SpanKind {
  if (span.rule) return "rule";
  if (span.gap) return "gap";
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
