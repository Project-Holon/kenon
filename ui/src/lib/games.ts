// Names for the cartridges kenon reads, from their game codes.

export type GameKind = "black" | "white" | "other";

/// Black or White, from a game code (IRB*: Black, IRA*: White).
export function gameOf(code: string): GameKind {
  if (code.startsWith("IRB")) return "black";
  if (code.startsWith("IRA")) return "white";
  return "other";
}

const LANGUAGES: Record<string, { short: string; name: string }> = {
  O: { short: "EN", name: "English" },
  E: { short: "EN", name: "English" },
  P: { short: "EN", name: "English" },
  F: { short: "FR", name: "Français" },
  D: { short: "DE", name: "Deutsch" },
  I: { short: "IT", name: "Italiano" },
  S: { short: "ES", name: "Español" },
  J: { short: "JA", name: "日本語" },
};

/// The language of a game code, in that language.
export function languageOf(code: string): { short: string; name: string } | null {
  return LANGUAGES[code[3] ?? ""] ?? null;
}

/// "Pokémon White" and the like.
export function gameName(kind: GameKind): string {
  return kind === "black" ? "Pokémon Black" : kind === "white" ? "Pokémon White" : "Another game";
}

/// A short badge: "WHITE · FR".
export function gameBadge(code: string): string {
  const kind = gameOf(code);
  if (kind === "other") return code;
  const lang = languageOf(code);
  return `${kind.toUpperCase()}${lang ? ` · ${lang.short}` : ""}`;
}

/// The legendary on each game's box: Zekrom (644) on Black, Reshiram (643)
/// on White.
export function legendaryOf(kind: GameKind): number | null {
  return kind === "black" ? 644 : kind === "white" ? 643 : null;
}

/// "2 h ago", "Yesterday", "Oct 18"; empty when not known.
export function since(seconds: number, now = Date.now() / 1000): string {
  if (!seconds) return "";
  const d = Math.max(0, now - seconds);
  if (d < 60) return "Just now";
  if (d < 3600) return `${Math.floor(d / 60)} min ago`;
  if (d < 86400) return `${Math.floor(d / 3600)} h ago`;
  if (d < 2 * 86400) return "Yesterday";
  if (d < 7 * 86400) return `${Math.floor(d / 86400)} days ago`;
  return new Date(seconds * 1000).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}
