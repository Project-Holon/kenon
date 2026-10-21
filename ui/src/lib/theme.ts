// Night (Zekrom), day (Reshiram), or the system's setting. Kept per viewer.

export type Theme = "dark" | "light" | "system";

const KEY = "kenon.theme";
const media = typeof matchMedia === "function" ? matchMedia("(prefers-color-scheme: light)") : null;

export function savedTheme(): Theme {
  try {
    const t = localStorage.getItem(KEY);
    if (t === "dark" || t === "light" || t === "system") return t;
  } catch {
    // Storage blocked: fall back to the system.
  }
  return "system";
}

function apply(theme: Theme) {
  const resolved = theme === "system" ? (media?.matches ? "light" : "dark") : theme;
  document.documentElement.dataset.theme = resolved;
}

export function setTheme(theme: Theme) {
  try {
    localStorage.setItem(KEY, theme);
  } catch {
    // Not saved; it lasts for this session.
  }
  apply(theme);
}

/// Applies the saved theme and follows the system while "system" is chosen.
export function initTheme() {
  apply(savedTheme());
  media?.addEventListener("change", () => {
    if (savedTheme() === "system") apply("system");
  });
}

export const THEMES: [Theme, string][] = [
  ["dark", "Zekrom (night)"],
  ["light", "Reshiram (day)"],
  ["system", "Follow system"],
];
