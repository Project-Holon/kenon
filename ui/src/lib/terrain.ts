// Tile kinds: what the low half of a tile's attribute word means to the
// field code (aletheia ov021/map_attr_types.h), with a colour per family.

export const FLAG_BLOCKED = 1;

type Family = "plain" | "grass" | "dark" | "tall" | "water" | "deep" | "sand" | "snow" | "ice" | "ledge" | "cave" | "slope" | "special" | "none";

const KINDS: Record<number, [string, Family]> = {
  0x00: ["Plain", "plain"],
  0x02: ["Puddle", "water"],
  0x04: ["Grass", "grass"],
  0x05: ["Grass (B)", "grass"],
  0x06: ["Dark grass", "dark"],
  0x07: ["Dark grass (B)", "dark"],
  0x08: ["Tall grass", "tall"],
  0x09: ["Tall dark grass", "tall"],
  0x0a: ["Cave floor", "cave"],
  0x0b: ["Sand (B)", "sand"],
  0x0c: ["Sand", "sand"],
  0x0e: ["Snow", "snow"],
  0x0f: ["Deep snow", "snow"],
  0x10: ["Ice", "ice"],
  0x14: ["Rippling water", "water"],
  0x17: ["Splashing water", "water"],
  0x18: ["Ice (B)", "ice"],
  0x19: ["Sand (D)", "sand"],
  0x1b: ["Mirror floor", "special"],
  0x1c: ["Splashing water (B)", "water"],
  0x1d: ["Hole", "special"],
  0x1e: ["Mud", "sand"],
  0x20: ["No wild Pokemon", "plain"],
  0x21: ["Grass (C)", "grass"],
  0x22: ["Dark grass (C)", "dark"],
  0x23: ["Sand (C)", "sand"],
  0x30: ["Electric floor", "special"],
  0x32: ["Electric panel", "special"],
  0x33: ["No running", "plain"],
  0x3d: ["Water", "water"],
  0x3e: ["Water (B)", "water"],
  0x3f: ["Deep water", "deep"],
  0x40: ["Waterfall", "deep"],
  0x41: ["Long jump edge", "ledge"],
  0x42: ["Water (C)", "water"],
  0x43: ["Deep water (B)", "deep"],
  0x44: ["Long jump edge (B)", "ledge"],
  0x51: ["Slope", "slope"],
  0x52: ["Slope", "slope"],
  0x53: ["Slope", "slope"],
  0x54: ["Slope", "slope"],
  0x55: ["Slope", "slope"],
  0x56: ["Slope", "slope"],
  0x57: ["Slope", "slope"],
  0x58: ["Slope", "slope"],
  0x72: ["Ledge, jump east", "ledge"],
  0x73: ["Ledge, jump west", "ledge"],
  0x74: ["Ledge, jump north", "ledge"],
  0x75: ["Ledge, jump south", "ledge"],
  0x7c: ["No interaction", "plain"],
  0xbe: ["Deep sand", "sand"],
  0xbf: ["Deep sand (B)", "sand"],
  0xd4: ["Camera step", "special"],
  0xff: ["Nothing", "none"],
};

const COLORS: Record<Family, [number, number, number]> = {
  plain: [196, 190, 172],
  grass: [104, 184, 96],
  dark: [56, 132, 64],
  tall: [40, 104, 48],
  water: [88, 152, 232],
  deep: [40, 88, 184],
  sand: [224, 200, 128],
  snow: [240, 244, 252],
  ice: [168, 224, 240],
  ledge: [168, 112, 64],
  cave: [140, 124, 108],
  slope: [184, 160, 120],
  special: [200, 120, 216],
  none: [0, 0, 0],
};

export function kindName(terrain: number): string {
  if (terrain >= 0xd6 && terrain <= 0xe2) return "Special floor";
  return KINDS[terrain]?.[0] ?? `Kind ${terrain}`;
}

function family(terrain: number): Family {
  if (terrain >= 0xd6 && terrain <= 0xe2) return "special";
  return KINDS[terrain]?.[1] ?? "plain";
}

// The colour of a tile: its kind's family, darker and redder when blocked.
// Unnamed kinds get their own tint so they stay apart.
export function tileRgb(terrain: number, flags: number): [number, number, number] {
  let [r, g, b] = COLORS[family(terrain)];
  if (!(terrain in KINDS) && !(terrain >= 0xd6 && terrain <= 0xe2)) {
    const h = (terrain * 47) % 360;
    [r, g, b] = hsl(h, 0.35, 0.68);
  }
  if (flags & FLAG_BLOCKED) return [Math.round(r * 0.45 + 70), Math.round(g * 0.35), Math.round(b * 0.35)];
  return [r, g, b];
}

export function tileCss(terrain: number, flags: number): string {
  const [r, g, b] = tileRgb(terrain, flags);
  return `rgb(${r} ${g} ${b})`;
}

function hsl(h: number, s: number, l: number): [number, number, number] {
  const k = (n: number) => (n + h / 30) % 12;
  const a = s * Math.min(l, 1 - l);
  const f = (n: number) => l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
  return [Math.round(f(0) * 255), Math.round(f(8) * 255), Math.round(f(4) * 255)];
}

export function tileLabel(terrain: number, flags: number): string {
  if (terrain < 0) return "Outside the map";
  return `${kindName(terrain)}${flags & FLAG_BLOCKED ? ", blocked" : ""}`;
}
