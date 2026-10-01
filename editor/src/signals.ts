// Shared signals: the module's numbers worked out once and named, for any
// page on it to draw. Stored signals in the code and the files, shared in the
// window, since sharing is what they are for.
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

import { deleteSignal, newPageId, savePage, saveSignal } from "./api";
import { bindingEditor, iconButton } from "./binding";
import { confirmAction } from "./confirm";
import { dismiss, dismissed, duplicates, pagesIn, pagesOf, placeOf, repoint } from "./duplicates";
import type { Duplicate } from "./duplicates";
import { noteEditor } from "./note";
import { signalsChanged, whereShown } from "./pages";
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
  /** Every device with this display, whose screen a page can be checked on. */
  devicesFor: (display: string) => string[];
  /** A device's name as the window shows it. */
  nameOf: (key: string) => string;
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
 * The Shared Signals section, and whether it holds edits not yet saved, which
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
  const banner = el("div", { class: "cautions", hidden: "" });
  const add = el("button", { class: "add small" }, "+ a shared number");
  const addLamp = el("button", { class: "add small" }, "+ shared lamp conditions");

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
                "About shared lamp conditions",
                "Conditions written once, for every lamp that repeats one cockpit " +
                  'lamp: choose "Use a shared signal" on each lamp. Each test ' +
                  "lights the lamp or leaves it off, and a scale dims it. Every " +
                  "lamp keeps its own brightness, so one can be full and another " +
                  "dimmer from the same conditions.",
              )
            : infoIcon(
                "About shared signals",
                "A number made once, for any page on this aircraft to draw: pick " +
                  '"a shared signal" as a piece of a field. Each part reads one ' +
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
          `Delete the shared signal ${w.signal.name}?` +
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

  /**
   * Readings on the module's pages that shape one signal the same way, as a
   * banner offering to make them one shared signal. It lives until its × puts
   * it away or every group in it has been made into one; a group put away
   * stays away, and only a new one brings the banner back.
   */
  const drawBanner = (): void => {
    const away = dismissed(module);
    const groups = book.broken ? [] : duplicates(book.saved).filter((d) => !away.has(d.key));
    banner.hidden = groups.length === 0;
    if (groups.length === 0) {
      banner.replaceChildren();
      return;
    }
    const close = iconButton("cancel", "✕", "Put this away", () => {
      dismiss(
        module,
        groups.map((d) => d.key),
      );
      drawBanner();
    });
    banner.replaceChildren(
      el(
        "div",
        { class: "condition-view" },
        el(
          "div",
          { class: "grow" },
          el("strong", {}, "These readings could each be one shared signal"),
          el(
            "div",
            { class: "meta" },
            "Each signal below is shaped the same way in more than one reading. Made one shared signal, it is shaped once, and every reading draws it as it does now.",
          ),
        ),
        close,
      ),
      ...groups.map(groupRow),
    );
  };

  /** One group in the banner: what it reads, where, and the button that makes it one. */
  const groupRow = (d: Duplicate): HTMLElement => {
    const source = d.term.source ?? "";
    const named = ctx.signals.find((s) => s.id === source)?.description;
    const open = pagesIn(d)
      .filter((p) => [...book.editing.values()].some((e) => e.page.id === p.id))
      .map((p) => p.name);
    const make = el("button", { class: "add small" }, "Make a shared signal");
    if (open.length > 0) {
      make.disabled = true;
      make.title = `Close ${open.join(", ")} first: saving would replace what is open there.`;
    }
    make.addEventListener("click", () => {
      make.disabled = true;
      void consolidate(d, named ?? source)
        .catch((e: unknown) => ctx.fail("Making the shared signal", e))
        .finally(() => (make.disabled = false));
    });
    return el(
      "div",
      { class: "caution" },
      el("code", {}, source),
      named ? ` ${named}, ` : " ",
      `in ${d.uses.length} readings on ${pagesOf(d).join(", ")}. `,
      make,
    );
  };

  /**
   * Make `d` one shared signal once the user confirms, listing every reading
   * it changes and every slot showing its pages, which is where to look.
   * The signal is saved first, since a page drawing it cannot pass its check
   * until it is.
   */
  const consolidate = async (d: Duplicate, name: string): Promise<void> => {
    const pages = pagesIn(d);
    // Each page with the readings on it that change, and every slot showing
    // it: a page is shared, so those are all the places to look after.
    const where = pages.map((p) => {
      const shown = whereShown(book, ctx.profile(), ctx.nameOf, p.id);
      const head =
        shown.length > 0
          ? `${p.name} page, on ${shown.join("; ")}:`
          : `${p.name} page, for the ${p.display}, not on any panel yet:`;
      const readings = d.uses.filter((u) => u.page.id === p.id).map((u) => `- ${placeOf(u)}`);
      return [head, ...readings].join("\n");
    });
    const ok = await confirmAction(
      `Make ${name} one shared signal?` +
        `\n\nIt is saved as a shared signal, and these ${d.uses.length} readings draw it instead, each as it does now:` +
        `\n\n${where.join("\n\n")}` +
        "\n\nThe pages are saved too, which changes them in every slot above.",
      "Confirm",
    );
    if (!ok) return;

    const id = await newPageId(avoid());
    const signal: StoredSignal = {
      id,
      name: freeName(name, id),
      terms: [structuredClone(d.term)],
      conditions: [],
    };
    let view = await saveSignal(module, signal);

    // All or nothing: a page its check refuses puts back the ones already
    // saved and takes the signal away again, so nothing is left half done.
    const saved: { page: Page; device: string }[] = [];
    try {
      for (const page of pages) {
        const device = ctx.devicesFor(page.display)[0];
        if (!device)
          throw new Error(
            `no panel here draws the ${page.display}, so ${page.name} cannot be checked`,
          );
        const changed = structuredClone(page);
        repoint(d, id, changed);
        view = await savePage(ctx.profile(), changed, device);
        saved.push({ page, device });
      }
    } catch (e) {
      try {
        for (const s of saved) view = await savePage(ctx.profile(), s.page, s.device);
        view = await deleteSignal(module, id);
      } catch (undo) {
        signalsChanged(book, view);
        redraw();
        throw new Error(
          `${e instanceof Error ? e.message : String(e)}\n\nPutting back what was already saved failed too: ${undo instanceof Error ? undo.message : String(undo)}`,
        );
      }
      signalsChanged(book, view);
      redraw();
      throw new Error(`${e instanceof Error ? e.message : String(e)}\n\nNothing was changed.`);
    }
    signalsChanged(book, view);
    redraw();
    ctx.changed();
    ctx.tell(
      `Made ${signal.name}, a shared signal, and saved ${pagesOf(d).join(", ")} with ${d.uses.length} readings drawing it.`,
    );
  };

  const redraw = (): void => {
    lead.textContent = book.broken
      ? `The page file for ${module} would not load, so its shared signals cannot be edited until it is fixed: ${book.broken}`
      : `Numbers made once and named, for any page on ${module} to draw. Each is saved on its own, and every profile on ${module} shares it.`;
    add.disabled = book.broken !== null;
    addLamp.disabled = book.broken !== null;
    drawBanner();
    list.replaceChildren(...shown().map(card));
  };

  /**
   * Ids a new signal must avoid: the signals and the pages not saved yet,
   * since a page and a signal never share an id.
   */
  const avoid = (): string[] => [
    ...[...working.values()].filter((w) => w.fresh).map((w) => w.signal.id),
    ...[...book.editing.values()].filter((e) => e.fresh).map((e) => e.page.id),
  ];

  const start = (lamp: boolean): void => {
    void newPageId(avoid()).then(
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
  // A page saved or deleted on any screen can make or end a duplicate.
  book.sections.push(drawBanner);
  addLamp.addEventListener("click", () => start(true));

  redraw();
  return {
    box: el(
      "section",
      { class: "device-group" },
      el("h2", {}, "Shared Signals"),
      lead,
      banner,
      list,
      el("div", { class: "chain-add" }, add, addLamp),
    ),
    unsaved: () => [...working.values()].some(unsaved),
  };
}
