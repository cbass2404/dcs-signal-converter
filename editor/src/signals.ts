// Stored signals: the module's numbers worked out once and named, for any
// page on it to draw.
//
// First on the profile page, above every panel, because a page draws from
// them and they are easy to miss below eighteen lamps. Like a page, each one
// belongs to the module's library rather than to the profile open here, so
// it is saved on its own and every profile on the module shares it. The
// profile's Save writes none of this.
//
// A signal is a short chain of parts, each one DCS-BIOS signal shaped exactly
// the way a reading is, laid side by side: the F-16's three fuel drums, each
// rounded down and wrapped at 10, read as one number. How it is drawn, its
// words and colours, is the piece's that draws it.

import { deleteSignal, newPageId, saveSignal } from "./api";
import { confirmAction } from "./confirm";
import { noteEditor } from "./note";
import { signalsChanged } from "./pages";
import type { PageBook } from "./pages";
import { termControls } from "./readout";
import { infoIcon } from "./typeahead";
import type { Page, SignalView, Span, StoredSignal } from "./types";

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
}

/** One signal as the window holds it while it is being edited. */
interface Working {
  signal: StoredSignal;
  /** As last saved, to tell an edit from none. */
  baseline: string;
  /** Made here and never saved. */
  fresh: boolean;
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
  const add = el("button", { class: "add small" }, "+ a stored signal");

  const hold = (signal: StoredSignal, fresh: boolean): Working => {
    const w = { signal: structuredClone(signal), baseline: JSON.stringify(signal), fresh };
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

  const card = (w: Working): HTMLDetailsElement => {
    const box = el("details", { class: "device" }) as HTMLDetailsElement;
    box.open = opened.has(w.signal.id);
    box.addEventListener("toggle", () => {
      if (box.open) opened.add(w.signal.id);
      else opened.delete(w.signal.id);
    });
    const count = el("span", { class: "meta" });
    const summary = el("summary", {}, el("span", { class: "name" }), count);
    const body = el("div", { class: "display pages" });
    box.append(summary, body);

    const usedBy = (): string[] =>
      book.saved.filter((p) => draws(p, w.signal.id)).map((p) => p.name);
    const refreshHead = (): void => {
      const name = summary.querySelector(".name");
      if (name) name.textContent = w.signal.name || "unnamed";
      const parts = w.signal.terms.length;
      const users = usedBy();
      count.textContent =
        `${parts} part${parts === 1 ? "" : "s"}` +
        (users.length ? ` · drawn on ${users.join(", ")}` : " · not drawn on any page") +
        (unsaved(w) ? " · not saved" : "");
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
          infoIcon(
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
        noteEditor(w.signal, "signal", edited),
      );

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
      body.append(chain, el("div", { class: "chain-add" }, more));

      const save = el("button", { class: "primary" }, "Save signal");
      save.disabled = book.broken !== null;
      save.addEventListener("click", () => {
        void saveSignal(module, w.signal).then(
          (view) => {
            w.baseline = JSON.stringify(w.signal);
            w.fresh = false;
            signalsChanged(book, view);
            redraw();
            ctx.changed();
            ctx.tell(`Saved ${w.signal.name.trim()}. Every page drawing it draws the change.`);
          },
          (e: unknown) => ctx.fail("Saving the signal", e),
        );
      });
      const undo = el("button", {}, w.fresh ? "Discard" : "Undo changes");
      undo.addEventListener("click", () => {
        if (w.fresh) working.delete(w.signal.id);
        else w.signal = JSON.parse(w.baseline) as StoredSignal;
        redraw();
      });
      const remove = el("button", { class: "danger" }, "Delete");
      remove.hidden = w.fresh;
      remove.addEventListener("click", () => {
        const users = usedBy();
        if (users.length) {
          ctx.fail(
            "Deleting the signal",
            `${w.signal.name} is drawn on ${users.join(", ")}. Take it off those pages first.`,
          );
          return;
        }
        void confirmAction(
          `Delete ${w.signal.name}?\n\nNo page draws it, and every profile on ${module} loses it.`,
          "Delete",
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
    return box;
  };

  const redraw = (): void => {
    lead.textContent = book.broken
      ? `The page file for ${module} would not load, so its stored signals cannot be edited until it is fixed: ${book.broken}`
      : `Numbers made once and named, for any page on ${module} to draw. Each is saved on its own, and every profile on ${module} shares it.`;
    add.disabled = book.broken !== null;
    list.replaceChildren(...shown().map(card));
  };

  add.addEventListener("click", () => {
    // Avoiding the pages not saved yet as well, since a page and a signal
    // never share an id.
    const avoid = [
      ...[...working.values()].filter((w) => w.fresh).map((w) => w.signal.id),
      ...[...book.editing.values()].filter((e) => e.fresh).map((e) => e.page.id),
    ];
    void newPageId(avoid).then(
      (id) => {
        hold({ id, name: freeName("New signal", id), terms: [{ source: "" }] }, true);
        opened.add(id);
        redraw();
      },
      (e: unknown) => ctx.fail("Adding a signal", e),
    );
  });

  redraw();
  return {
    box: el(
      "section",
      { class: "device-group" },
      el("h2", {}, "Stored Signals"),
      lead,
      list,
      el("div", { class: "chain-add" }, add),
    ),
    unsaved: () => [...working.values()].some(unsaved),
  };
}
