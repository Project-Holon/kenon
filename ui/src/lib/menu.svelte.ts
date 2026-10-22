// Right-click menus and short confirmations. Any `oncontextmenu` handler
// calls `openMenu(event, items)`; the menu itself (ContextMenu.svelte) is
// mounted on the page the first time one is needed.
import { mount } from "svelte";
import ContextMenu from "./ContextMenu.svelte";

export type MenuItem =
  | {
      label: string;
      run: () => unknown;
      /// A shortcut shown at the right, for the user to read only.
      hint?: string;
      disabled?: boolean;
    }
  | "-";

export const menu = $state({ open: false, x: 0, y: 0, items: [] as MenuItem[], host: null as HTMLElement | null });

export const toasts = $state<{ text: string; bad: boolean; id: number }[]>([]);

let mounted = false;
function ensure() {
  if (mounted) return;
  mounted = true;
  mount(ContextMenu, { target: document.body });
}

/// Items without leading, trailing or doubled separators.
function tidy(items: MenuItem[]): MenuItem[] {
  const out: MenuItem[] = [];
  for (const item of items) {
    if (item === "-" && (out.length === 0 || out[out.length - 1] === "-")) continue;
    out.push(item);
  }
  if (out[out.length - 1] === "-") out.pop();
  return out;
}

/// Opens the menu at the pointer. Text fields get the cut, copy and paste
/// menu (installMenuFallback), so a handler that wants that checks
/// `isTextField` first and leaves the event alone.
export function openMenu(e: MouseEvent, items: MenuItem[]) {
  const list = tidy(items);
  if (!list.length) return;
  e.preventDefault();
  e.stopPropagation();
  ensure();
  menu.x = e.clientX;
  menu.y = e.clientY;
  menu.items = list;
  // A modal dialog hides the page behind it: the menu goes inside it.
  menu.host = (e.target as Element | null)?.closest?.("dialog") ?? null;
  menu.open = true;
}

export function closeMenu() {
  menu.open = false;
}

/// True for the targets that have a system menu of their own.
export function isTextField(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  if (!el?.tagName) return false;
  return (
    el.isContentEditable ||
    el.tagName === "TEXTAREA" ||
    (el.tagName === "INPUT" && !["checkbox", "radio", "range", "button", "color"].includes((el as HTMLInputElement).type))
  );
}

/// True when the system menu fits better: any field but a text area, or a
/// text area with text selected (cut, copy, paste).
export function keepNative(e: MouseEvent): boolean {
  const el = e.target as HTMLTextAreaElement | null;
  if (!isTextField(el)) return false;
  if (el!.tagName !== "TEXTAREA") return true;
  return el!.selectionStart !== el!.selectionEnd;
}

/// The menu for a text field: cut, copy, paste, select all. Typing
/// commands keep the field's undo history and its input events.
function textMenu(e: MouseEvent, el: HTMLInputElement | HTMLTextAreaElement | HTMLElement) {
  const field = "value" in el ? (el as HTMLInputElement) : null;
  const selected = field
    ? field.value.slice(field.selectionStart ?? 0, field.selectionEnd ?? 0)
    : (window.getSelection()?.toString() ?? "");
  const editable = field ? !field.readOnly && !field.disabled : el.isContentEditable;
  openMenu(e, [
    {
      label: "Cut",
      hint: "Ctrl+X",
      disabled: !selected || !editable,
      run: async () => {
        await navigator.clipboard.writeText(selected).catch(() => {});
        el.focus();
        document.execCommand("delete");
      },
    },
    {
      label: "Copy",
      hint: "Ctrl+C",
      disabled: !selected,
      run: () => navigator.clipboard.writeText(selected).catch(() => toast("Could not copy.", true)),
    },
    {
      label: "Paste",
      hint: "Ctrl+V",
      disabled: !editable,
      run: async () => {
        try {
          const text = await navigator.clipboard.readText();
          el.focus();
          document.execCommand("insertText", false, text);
        } catch {
          toast("Press Ctrl+V to paste.", true);
        }
      },
    },
    "-",
    {
      label: "Select all",
      hint: "Ctrl+A",
      run: () => {
        el.focus();
        if (field) field.select();
        else document.execCommand("selectAll");
      },
    },
  ]);
}

/// Replaces the browser's own right-click menu everywhere: a text field gets
/// a cut, copy and paste menu, anything else no menu. Views with menus of
/// their own open them first (openMenu stops the event), and the code editor
/// keeps its own.
export function installMenuFallback() {
  window.addEventListener("contextmenu", (e) => {
    if (e.defaultPrevented) return;
    const el = e.target as HTMLElement | null;
    if (el?.closest?.(".monaco-editor")) return;
    if (isTextField(el)) {
      textMenu(e, el!);
      return;
    }
    e.preventDefault();
  });
}

let nextToast = 0;

/// A short note at the bottom of the window.
export function toast(text: string, bad = false) {
  ensure();
  const id = nextToast++;
  toasts.push({ text, bad, id });
  setTimeout(
    () => {
      const at = toasts.findIndex((t) => t.id === id);
      if (at >= 0) toasts.splice(at, 1);
    },
    bad ? 6000 : 2500,
  );
}
