// The Settings dialog, behind the gear on the Profiles page.
//
// What belongs to the PC rather than to any profile: the window's theme and
// the key or controller button held to swap pages, both kept in
// settings.json, and the two
// things on this page that are not about one profile, Import and Manage
// Converter, moved here to keep the header short. Laid out like converter.ts.

import { controllerCapture, controllerCaptureCancel, settingsRead, settingsSave } from "./api";
import type {
  ControllerButton,
  ModifierKey,
  PageModifier,
  Settings,
  SettingsView,
  Theme,
} from "./types";

/** What the dialog closed on: nothing, or one of the dialogs it hands on to. */
export type SettingsExit = null | "import" | "converter";

/** Draw the window in a theme. System leaves it to Windows, as the stylesheet always did. */
export function applyTheme(theme: Theme): void {
  if (theme === "system") document.documentElement.removeAttribute("data-theme");
  else document.documentElement.dataset.theme = theme;
}

/** Read the settings and draw the window in their theme, before the first screen. */
export async function loadTheme(): Promise<void> {
  try {
    applyTheme((await settingsRead()).theme);
  } catch {
    // The window follows Windows, as it did before there was a choice.
  }
}

/** A labelled dropdown, as the pickers lay one out. */
function choice<T extends string>(
  label: string,
  options: [T, string][],
  value: T,
): [HTMLLabelElement, HTMLSelectElement] {
  const field = document.createElement("label");
  field.className = "field";
  field.append(label);
  const select = document.createElement("select");
  for (const [v, text] of options) {
    const option = document.createElement("option");
    option.value = v;
    option.textContent = text;
    select.append(option);
  }
  select.value = value;
  field.append(select);
  return [field, select];
}

function note(text: string): HTMLParagraphElement {
  const p = document.createElement("p");
  p.className = "meta";
  p.textContent = text;
  return p;
}

/**
 * Open the dialog. Resolves once it is closed, with the dialog to open next
 * if Import or Manage Converter was pressed.
 *
 * A change is saved as it is made, so there is no Save to forget: the theme
 * shows at once, and a running converter picks up the modifier within about a
 * second, as it picks up a saved profile.
 */
export function showSettings(): Promise<SettingsExit> {
  return new Promise((resolve) => {
    void settingsRead().then(
      (view) => open(view, view.problem, resolve),
      (e) =>
        open(
          { page_modifier: "ctrl", theme: "system", problem: String(e), modifier_missing: false },
          String(e),
          resolve,
        ),
    );
  });
}

/** The dropdown's value for a modifier: the key's name, or "button". */
function modifierChoice(m: PageModifier): ModifierKey | "button" {
  return typeof m === "string" ? m : "button";
}

function buttonText(b: ControllerButton): string {
  return `${b.name} button ${b.button}`;
}

function missingText(b: ControllerButton): string {
  return `${b.name} not found. Page swapping is disabled until it's back or another modifier is selected.`;
}

const KEY_NOTE =
  "Hold it alone: with a second modifier held too, the press is left to DCS and nothing swaps. DCS still sees every press, so leave the combination you pick unbound in DCS, or one press will swap the page and do whatever it is bound to.";

const BUTTON_NOTE =
  "Pick the button you set as a modifier in DCS's controls, so DCS keeps it and a page key as a combination of their own, as it does Ctrl and a key. Hold it with no Ctrl, Shift or Alt. Its number is the one DCS gives it. A button DCS binds to something else does that too, on every swap.";

function open(
  current: SettingsView,
  problem: string | null | undefined,
  resolve: (exit: SettingsExit) => void,
): void {
  const settings: Settings = { page_modifier: current.page_modifier, theme: current.theme };
  const dialog = document.createElement("dialog");
  dialog.className = "picker confirm settings";

  const h2 = document.createElement("h2");
  h2.textContent = "Settings";
  dialog.append(h2);

  const state = note("");
  if (problem) {
    state.textContent = `The settings file could not be read, so these are the defaults. Changing one writes a new file. ${problem}`;
    state.classList.add("bad");
  }
  dialog.append(state);

  const save = (): void => {
    state.textContent = "";
    state.classList.remove("bad");
    void settingsSave(settings).catch((e: unknown) => {
      state.textContent = e instanceof Error ? e.message : String(e);
      state.classList.add("bad");
    });
  };

  const [themeField, theme] = choice<Theme>(
    "Appearance",
    [
      ["system", "Follow Windows"],
      ["light", "Light"],
      ["dark", "Dark"],
    ],
    settings.theme,
  );
  theme.addEventListener("change", () => {
    settings.theme = theme.value as Theme;
    applyTheme(settings.theme);
    save();
  });
  dialog.append(themeField);

  const [modifierField, modifier] = choice<ModifierKey | "button">(
    "Page keys",
    [
      ["ctrl", "Ctrl + page key"],
      ["shift", "Shift + page key"],
      ["alt", "Alt + page key"],
      ["button", "Controller button + page key..."],
    ],
    modifierChoice(settings.page_modifier),
  );
  const buttonOption = modifier.querySelector<HTMLOptionElement>('option[value="button"]')!;
  const missing = note("");
  missing.classList.add("bad");
  const modifierNote = note("");
  const another = document.createElement("button");
  another.textContent = "Press another button...";
  const anotherRow = document.createElement("div");
  anotherRow.append(another);

  // What the dropdown, its notes and Press another say for the setting now.
  const showModifier = (): void => {
    const m = settings.page_modifier;
    buttonOption.textContent =
      typeof m === "string" ? "Controller button + page key..." : `${buttonText(m)} + page key`;
    modifier.value = modifierChoice(m);
    modifierNote.textContent = typeof m === "string" ? KEY_NOTE : BUTTON_NOTE;
    anotherRow.hidden = typeof m === "string";
  };
  showModifier();
  const pm = settings.page_modifier;
  missing.hidden = !(current.modifier_missing && typeof pm !== "string");
  if (!missing.hidden && typeof pm !== "string") missing.textContent = missingText(pm);

  let waiting = false;
  // Wait for a button on any controller. Nothing pressed in time leaves the
  // modifier as it was.
  const capture = async (): Promise<void> => {
    if (waiting) return;
    waiting = true;
    modifier.disabled = true;
    another.disabled = true;
    modifierNote.textContent =
      "Press the button to hold with page keys, on any controller. Waiting 10 seconds...";
    let pressed: ControllerButton | null = null;
    let failed: string | null = null;
    try {
      pressed = await controllerCapture();
    } catch (e) {
      failed = e instanceof Error ? e.message : String(e);
    }
    waiting = false;
    if (done) return;
    modifier.disabled = false;
    another.disabled = false;
    if (pressed) {
      settings.page_modifier = pressed;
      missing.hidden = true;
      save();
    }
    showModifier();
    if (!pressed) {
      modifierNote.textContent =
        failed ?? "No button was pressed, so the page modifier is unchanged.";
    }
  };

  modifier.addEventListener("change", () => {
    if (modifier.value === "button") {
      void capture();
      return;
    }
    settings.page_modifier = modifier.value as ModifierKey;
    missing.hidden = true;
    showModifier();
    save();
  });
  another.addEventListener("click", () => void capture());
  dialog.append(modifierField, missing, modifierNote, anotherRow);

  const importButton = document.createElement("button");
  importButton.textContent = "Import profile...";
  const converterButton = document.createElement("button");
  converterButton.textContent = "Manage Converter...";
  const close = document.createElement("button");
  close.className = "primary";
  close.textContent = "Close";

  // The two hand-offs to the left, Close alone on the right where every
  // dialog's action sits.
  const actions = document.createElement("div");
  actions.className = "actions";
  actions.append(importButton, converterButton, close);
  dialog.append(actions);

  let done = false;
  const finish = (exit: SettingsExit): void => {
    if (done) return;
    done = true;
    if (waiting) void controllerCaptureCancel();
    dialog.close();
    dialog.remove();
    resolve(exit);
  };

  close.addEventListener("click", () => finish(null));
  importButton.addEventListener("click", () => finish("import"));
  converterButton.addEventListener("click", () => finish("converter"));
  dialog.addEventListener("close", () => finish(null));
  dialog.addEventListener("click", (e) => {
    if (e.target !== dialog) return;
    const r = dialog.getBoundingClientRect();
    const inside =
      e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom;
    if (!inside) finish(null);
  });

  document.body.append(dialog);
  dialog.showModal();
  close.focus();
}
