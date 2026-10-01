// The gauge tables from docs/gauges.json, offered on a converted reading.
//
// Bundled with the editor rather than fetched, so the tables are there with no
// network and match the release they shipped in. A table is copied into the
// field when it is picked: the field then owns its rows like any others, and a
// later gauges.json changes nothing a profile already holds.

import doc from "../../docs/gauges.json";
import type { Conversion } from "./types";

/** One uneven gauge, as tools/gen_gauges.py writes it. */
export interface GaugeTable {
  /** The DCS-BIOS signal the needle is sent on. */
  id: string;
  description: string;
  unit: string;
  notes: string[];
  conversions: Conversion[];
}

interface GaugeAircraft {
  /** The DCS-BIOS module, which is the signal catalogue's and a profile's. */
  bios_module: string;
  gauges: GaugeTable[];
}

const BY_MODULE = new Map<string, GaugeTable[]>(
  (doc.aircraft as unknown as GaugeAircraft[]).map((a) => [a.bios_module, a.gauges]),
);

/** Every table for a module, in the order gauges.json lists them. */
export function gaugeTables(module: string): GaugeTable[] {
  return BY_MODULE.get(module) ?? [];
}

/** The table for one signal, where the module has one. */
export function gaugeTable(module: string, id: string): GaugeTable | undefined {
  return gaugeTables(module).find((g) => g.id === id);
}

/** The rows a field gets from a table: copies, so editing one leaves the table alone. */
export function tableRows(table: GaugeTable): Conversion[] {
  return table.conversions.map((c) => ({
    raw: [c.raw[0], c.raw[1]],
    reads: [c.reads[0], c.reads[1]],
  }));
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
