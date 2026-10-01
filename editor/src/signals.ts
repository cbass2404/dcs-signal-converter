// Stored signals: the module's numbers worked out once and named, for any
// page on it to draw.
//
// First on the profile page, above every panel, because a page draws from
// them and they are easy to miss below eighteen lamps. Like a page, each one
// belongs to the module's library rather than to the profile open here, so
// it is saved on its own and every profile on the module shares it. The
// profile's Save writes none of this.
//
// A signal is one of two kinds. A number is a short chain of parts, each one
// DCS-BIOS signal shaped exactly the way a reading is, laid side by side: the
// F-16's three fuel drums, each rounded down and wrapped at 10, read as one
// number. How it is drawn, its words and colours, is the piece's that draws
// it. Lamp conditions are tests written as a lamp's are, for every lamp that
// repeats one cockpit lamp; each lamp keeps its own brightness.

import { deleteSignal, newPageId, saveSignal } from "./api";
import { bindingEditor, iconButton } from "./binding";
import { confirmAction } from "./confirm";
import { noteEditor } from "./note";
import { signalsChanged } from "./pages";
import type { PageBook } from "./pages";
import { termControls } from "./readout";
import { isLampSignal } from "./stored";
import { infoIcon } from "./typeahead";
import type { Binding, Led, Page, Profile, SignalView, Span, StoredSignal } from "./types";

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attrs: Record<string, string> = {},
  ...kids: (Node | string)[]
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, v);
  node.append(...kids);
  return node;
}

/** What the section needs from the profile page around it. */
export interface SignalContext {
  book: PageBook;
  /** The module's DCS-BIOS signals, which the parts read. */
  signals: SignalView[];
  /** Say something that happened, such as a save. */
  tell: (text: string) => void;
  /** Say something failed. */
  fail: (where: string, e: unknown) => void;
  /** The library changed, so the profile's pages want checking again. */
  changed: () => void;
  /** The profile open here, whose lamps may light by a signal. */
  profile: () => Profile;
}

/** One signal as the window holds it while it is being edited. */
interface Working {
  signal: StoredSignal;
  /** As last saved, to tell an edit from none. */
  baseline: string;
  /** Made here and never saved. */
  fresh: boolean;
  /** Lamp conditions rather than a number. Fixed when it is made. */
  lamp: boolean;
}

/**
 * The lamp a signal's conditions are written against while they are edited:
 * one that only turns on and off, so a test picked for a signal is a
 * threshold. A scale can still be chosen, for a dimmer.
 */
function standIn(name: string): Led {
  return {
    name,
    label: name,
    kind: "indicator",
    max: 1,
    on_value: 1,
    dimmable: false,
    verified: true,
    note: "",
    governs: [],
  } as unknown as Led;
}

const sameName = (a: string, b: string): boolean =>
  a.trim().toLowerCase() === b.trim().toLowerCase();

/** Whether any piece on `page` draws the stored signal `id`, switch cases included. */
function draws(page: Page, id: string): boolean {
  return page.fields.some((f) => JSON.stringify(f).includes(`"signal":${JSON.stringify(id)}`));
}

/**
 * The Stored Signals section, and whether it holds edits not yet saved, which
 * leaving the profile would lose.
 */
export function signalSection(ctx: SignalContext): {
  box: HTMLElement;
  unsaved: () => boolean;
} {
  const { book } = ctx;
  const module = book.module;
  const working = new Map<string, Working>();
  const opened = new Set<string>();
  const list = el("div", {});
  const lead = el("p", { class: "meta" });
  const add = el("button", { class: "add small" }, "+ a stored number");
  const addLamp = el("button", { class: "add small" }, "+ stored lamp conditions");

  const hold = (signal: StoredSignal, fresh: boolean, lamp = isLampSignal(signal)): Working => {
    const w = { signal: structuredClone(signal), baseline: JSON.stringify(signal), fresh, lamp };
    working.set(signal.id, w);
    return w;
  };
  const unsaved = (w: Working): boolean => w.fresh || JSON.stringify(w.signal) !== w.baseline;

  /** Every signal shown: the saved ones, as being edited where they are, then the new ones. */
  const shown = (): Working[] => {
    const saved = book.signals.map((s) => working.get(s.id) ?? hold(s, false));
    const fresh = [...working.values()].filter((w) => w.fresh);
    return [...saved, ...fresh];
  };

  const freeName = (name: string, except: string): string => {
    const taken = (n: string): boolean =>
      shown().some((w) => w.signal.id !== except && sameName(w.signal.name, n));
    if (!taken(name)) return name;
    for (let k = 2; ; k += 1) if (!taken(`${name} ${k}`)) return `${name} ${k}`;
  };

  /**
   * One signal: a line saying what it reads until its pencil is clicked, the
   * way a field reads as a sentence, and its editor while it is open or holds
   * changes not yet saved.
   */
  const card = (w: Working): HTMLElement => {
    // Where it is used: pages for a number, this profile's lamps for lamp
    // conditions. Lamps in other profiles are the converter's to refuse.
    const usedBy = (): string[] =>
      w.lamp
        ? ctx
            .profile()
            .bindings.filter((b) => b.signal === w.signal.id)
            .map((b) => b.led)
        : book.saved.filter((p) => draws(p, w.signal.id)).map((p) => p.name);
    const drawnOn = (): string => {
      const users = usedBy();
      if (w.lamp) return users.length ? `lights ${users.join(", ")}` : "lights no lamp here";
      return users.length ? `drawn on ${users.join(", ")}` : "not drawn on any page";
    };

    if (!opened.has(w.signal.id) && !unsaved(w)) {
      const tested = [
        ...new Set(
          [...w.signal.conditions, ...(w.signal.any_of ?? []).flatMap((b) => b.conditions)].map(
            (c) => c.source || "a condition with no signal yet",
          ),
        ),
      ];
      const sources = w.lamp
        ? tested
        : w.signal.terms.map((t) => t.source || "a part with no signal yet");
      const reads = w.lamp ? `tests ${sources.join(", ")}` : `reads ${sources.join(", then ")}`;
      const edit = iconButton("pencil", "\u270E", "Edit this signal", () => {
        opened.add(w.signal.id);
        redraw();
      });
      return el(
        "div",
        { class: "display" },
        el(
          "div",
          { class: "condition-view" },
          el(
            "div",
            { class: "grow" },
            el("strong", {}, w.signal.name),
            el("div", { class: "meta" }, `${reads} · ${drawnOn()}`),
            w.signal.note ? el("div", { class: "meta" }, w.signal.note) : "",
          ),
          edit,
        ),
      );
    }

    const body = el("div", { class: "display" });
    const state = el("span", { class: "meta" });
    // Undo with changes to undo, Close without, Discard for one never saved.
    const undo = el("button", {});
    const refreshHead = (): void => {
      state.textContent = drawnOn() + (unsaved(w) ? " · not saved" : "");
      undo.textContent = w.fresh ? "Discard" : unsaved(w) ? "Undo changes" : "Close";
    };
    const edited = (): void => refreshHead();

    const draw = (): void => {
      body.replaceChildren();
      const name = el("input", { type: "text", class: "span-text", value: w.signal.name });
      name.addEventListener("input", () => {
        w.signal.name = name.value;
        edited();
      });
      body.append(
        el(
          "div",
          { class: "display-head" },
          el("label", { class: "meta" }, "Name ", name),
          state,
          w.lamp
            ? infoIcon(
                "About stored lamp conditions",
                "Conditions written once, for every lamp that repeats one cockpit " +
                  'lamp: choose "Use a stored signal" on each lamp. Each test ' +
                  "lights the lamp or leaves it off, and a scale dims it. Every " +
                  "lamp keeps its own brightness, so one can be full and another " +
                  "dimmer from the same conditions.",
              )
            : infoIcon(
                "About stored signals",
                "A number made once, for any page on this aircraft to draw: pick " +
                  '"a stored signal" as a piece of a field. Each part reads one ' +
                  "signal and shapes it the way a reading does, and the parts are " +
                  "laid side by side, so three fuel drums, each rounded down and " +
                  "wrapped at 10, read as one number. Rounding each drum on its own " +
                  "is what stops a drum that is rolling counting twice. Words and " +
                  "colours belong to the piece that draws it.",
              ),
        ),
      );

      if (w.lamp) {
        body.append(
          bindingEditor({
            binding: w.signal as unknown as Binding,
            led: standIn(w.signal.name),
            signals: ctx.signals,
            targets: () => [],
            deviceName: (key) => key,
            conditionsOnly: true,
            onChange: edited,
            onCommit: edited,
          }),
          noteEditor(w.signal, "signal", edited, true),
        );
      }

      const chain = el("div", { class: "chain" });
      w.signal.terms.forEach((term, i) => {
        const span = term as Span;
        const up = el("button", { class: "icon", title: "Move this part earlier" }, "↑");
        up.disabled = i === 0;
        up.addEventListener("click", () => {
          w.signal.terms.splice(i - 1, 0, ...w.signal.terms.splice(i, 1));
          draw();
        });
        const down = el("button", { class: "icon", title: "Move this part later" }, "↓");
        down.disabled = i === w.signal.terms.length - 1;
        down.addEventListener("click", () => {
          w.signal.terms.splice(i + 1, 0, ...w.signal.terms.splice(i, 1));
          draw();
        });
        const drop = el("button", { class: "icon danger", title: "Remove this part" }, "×");
        drop.disabled = w.signal.terms.length === 1;
        drop.addEventListener("click", () => {
          w.signal.terms.splice(i, 1);
          draw();
        });
        chain.append(
          el(
            "div",
            { class: "span" },
            el(
              "div",
              { class: "span-head" },
              el("span", { class: "meta" }, `part ${i + 1}`),
              el("span", { class: "spacer" }),
              up,
              down,
              drop,
            ),
            el(
              "div",
              { class: "span-body" },
              ...termControls(span, ctx.signals, module, edited, draw),
            ),
          ),
        );
      });
      const more = el("button", { class: "add small" }, "+ a part");
      more.addEventListener("click", () => {
        w.signal.terms.push({ source: "" });
        draw();
      });
      // Last, after the parts, so it reads as the summary of what they make.
      if (!w.lamp) {
        body.append(
          chain,
          el("div", { class: "chain-add" }, more),
          noteEditor(w.signal, "signal", edited, true),
        );
      }

      const save = el("button", { class: "primary" }, "Save signal");
      save.disabled = book.broken !== null;
      save.addEventListener("click", () => {
        void saveSignal(module, w.signal).then(
          (view) => {
            w.baseline = JSON.stringify(w.signal);
            w.fresh = false;
            opened.delete(w.signal.id);
            signalsChanged(book, view);
            redraw();
            ctx.changed();
            ctx.tell(`Saved ${w.signal.name.trim()}. Every page drawing it draws the change.`);
          },
          (e: unknown) => ctx.fail("Saving the signal", e),
        );
      });
      undo.onclick = (): void => {
        if (w.fresh) working.delete(w.signal.id);
        else w.signal = JSON.parse(w.baseline) as StoredSignal;
        opened.delete(w.signal.id);
        redraw();
      };
      const remove = el("button", { class: "danger" }, "Delete");
      remove.hidden = w.fresh;
      remove.addEventListener("click", () => {
        // Laid out like deleting a page: what uses it, flagged, then that it
        // cannot be undone. While a page draws it there is only Close, since
        // every piece drawing it would be left drawing nothing.
        const places = usedBy().map((p) => `- ${p}`);
        const where = w.lamp ? "These lamps light by it" : "These pages draw it";
        void confirmAction(
          `Delete the stored signal ${w.signal.name}?` +
            (places.length > 0
              ? `\n\n${where}:\n${places.join("\n")}` +
                `\n\nTake it off those ${w.lamp ? "lamps" : "pages"} before it can be deleted.`
              : `\n\nEvery profile on ${module} loses it.`) +
            "\n\nDeletion cannot be undone.",
          "Delete",
          places,
          places.length > 0,
        ).then((ok) => {
          if (!ok) return;
          void deleteSignal(module, w.signal.id).then(
            (view) => {
              working.delete(w.signal.id);
              signalsChanged(book, view);
              redraw();
              ctx.changed();
              ctx.tell(`Deleted ${w.signal.name}.`);
            },
            (e: unknown) => ctx.fail("Deleting the signal", e),
          );
        });
      });
      body.append(el("div", { class: "chain-add page-actions" }, save, undo, remove));
      refreshHead();
    };
    draw();
    return body;
  };

  const redraw = (): void => {
    lead.textContent = book.broken
      ? `The page file for ${module} would not load, so its stored signals cannot be edited until it is fixed: ${book.broken}`
      : `Numbers made once and named, for any page on ${module} to draw. Each is saved on its own, and every profile on ${module} shares it.`;
    add.disabled = book.broken !== null;
    addLamp.disabled = book.broken !== null;
    list.replaceChildren(...shown().map(card));
  };

  const start = (lamp: boolean): void => {
    // Avoiding the pages not saved yet as well, since a page and a signal
    // never share an id.
    const avoid = [
      ...[...working.values()].filter((w) => w.fresh).map((w) => w.signal.id),
      ...[...book.editing.values()].filter((e) => e.fresh).map((e) => e.page.id),
    ];
    void newPageId(avoid).then(
      (id) => {
        const name = freeName(lamp ? "New lamp conditions" : "New signal", id);
        hold({ id, name, terms: lamp ? [] : [{ source: "" }], conditions: [] }, true, lamp);
        opened.add(id);
        redraw();
      },
      (e: unknown) => ctx.fail("Adding a signal", e),
    );
  };
  add.addEventListener("click", () => start(false));
  addLamp.addEventListener("click", () => start(true));

  redraw();
  return {
    box: el(
      "section",
      { class: "device-group" },
      el("h2", {}, "Stored Signals"),
      lead,
      list,
      el("div", { class: "chain-add" }, add, addLamp),
    ),
    unsaved: () => [...working.values()].some(unsaved),
  };
}
