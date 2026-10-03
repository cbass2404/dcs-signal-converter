// What each DCS gauge reads, from tools/gen_gauges.py, offered on a converted
// reading.
//
// Bundled with the editor rather than fetched, so the data is there with no
// network and matches the release it shipped in. It is copied into the field
// when it is picked: the field then owns its numbers like any others, and a
// later regeneration changes nothing a profile already holds.

import data from "./gauge-data.json";
import type { Conversion } from "./types";

/**
 * One gauge: an even one's range end to end, or an uneven one's rows. Parsed
 * from the module's files, not read off the dial, so it is a start the user
 * checks rather than an answer.
 */
export interface Gauge {
  /** The DCS-BIOS signal the needle is sent on. */
  id: string;
  description: string;
  /** The dial's unit, or empty where nobody has read it off the dial. */
  unit: string;
  /** What raw 0 and the signal's maximum read, on an even gauge. */
  reads?: [number, number];
  /** One row per section between two marks, on an uneven gauge. */
  conversions?: Conversion[];
}

/** An uneven gauge, which has rows to offer. */
export type GaugeTable = Gauge & { conversions: Conversion[] };

const BY_MODULE = new Map<string, Gauge[]>(
  Object.entries(data as unknown as Record<string, Gauge[]>),
);

/** The gauge a signal is, where the module's files say. */
export function gauge(module: string, id: string): Gauge | undefined {
  return BY_MODULE.get(module)?.find((g) => g.id === id);
}

/**
 * Every uneven gauge in a module, in id order. Only these are offered as
 * tables: an even one's two numbers are already in the boxes.
 */
export function gaugeTables(module: string): GaugeTable[] {
  return (BY_MODULE.get(module) ?? []).filter((g): g is GaugeTable => !!g.conversions);
}

/** The rows a field gets from a table: copies, so editing one leaves the table alone. */
export function tableRows(table: GaugeTable): Conversion[] {
  return table.conversions.map((c) => ({
    raw: [c.raw[0], c.raw[1]],
    reads: [c.reads[0], c.reads[1]],
  }));
}

/**
 * Convert a reading the way its gauge does: by the gauge's rows, or across
 * its range. False when the module's files say nothing about this signal.
 */
export function fillFromGauge(
  span: { reads?: [number, number]; conversions?: Conversion[] },
  module: string,
  id: string,
): boolean {
  const g = gauge(module, id);
  if (g?.conversions) span.conversions = tableRows(g as GaugeTable);
  else if (g?.reads) span.reads = [g.reads[0], g.reads[1]];
  else return false;
  return true;
}

/**
 * The table these rows still are, if any. Colour and size are left out of
 * the comparison: styling the low end of a fuel gauge red keeps it that gauge.
 */
export function matchingTable(
  module: string,
  rows: { raw: [number, number]; reads: [number, number] }[],
): GaugeTable | undefined {
  return gaugeTables(module).find(
    (g) =>
      g.conversions.length === rows.length &&
      g.conversions.every((c, i) => {
        const r = rows[i];
        return (
          !!r &&
          c.raw[0] === r.raw[0] &&
          c.raw[1] === r.raw[1] &&
          c.reads[0] === r.reads[0] &&
          c.reads[1] === r.reads[1]
        );
      }),
  );
}

/**
 * A reminder under a reading still holding numbers filled in from a gauge, or
 * null once the user has changed them. Parsed numbers can be in DCS's units
 * rather than the dial's, or a little off a mark.
 */
export function gaugeNote(
  module: string,
  id: string | undefined,
  span: { reads?: [number, number]; conversions?: Conversion[] },
): string | null {
  let from: Gauge | undefined;
  if (span.conversions?.length) from = matchingTable(module, span.conversions);
  else if (span.reads && id) {
    const g = gauge(module, id);
    if (g?.reads && g.reads[0] === span.reads[0] && g.reads[1] === span.reads[1]) from = g;
  }
  if (!from) return null;
  return from.unit
    ? `Filled in from DCS's gauge data, in ${from.unit}. Check it against the dial.`
    : "Filled in from DCS's gauge data, unit not checked. Check it against the dial.";
}
