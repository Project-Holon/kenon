// The right-click menu of a picture, for the graphics list and the editor.
import { archiveLabel } from "./archiveLabels";
import { animBank, asset, cellBank, revert, sheetData, type Layout, type Ref } from "./backend";
import { copyText, fileName, guarded, openInFiles, saveAssets, saveMember, savePng, saveText } from "./exporter";
import type { ImportTarget } from "./ImportPicture.svelte";
import { importPng } from "./importPng";
import { toast, type MenuItem } from "./menu.svelte";

/// What names a picture of an archive.
export interface Picture {
  arc: number;
  label: string;
  chr: number;
  pal: number | null;
  /// The palette bank the game adds (Pokemon icons).
  offset: number;
  /// Known cell bank: null for a plain picture, undefined to find it.
  cells?: number | null;
  anims?: number | null;
  /// Picture, cells, animation, multi-cell (and a multi-cell animation).
  rig?: number[] | null;
  /// A background screen drawn with this sheet.
  scr?: number;
}

const rigPath = (p: Picture, pal: number) =>
  [p.rig![0], pal, ...p.rig!.slice(1)].map((m) => `${p.arc}:${m}`).join("/");

/// The bridge path of the picture as the game draws it. `cols` is the width
/// in tiles of a plain sheet.
export function picturePath(p: Picture, cols = 8): string {
  if (p.scr !== undefined) return `screen/${p.arc}:${p.chr}/${p.arc}:${p.pal}/${p.arc}:${p.scr}`;
  if (p.pal === null) return "";
  if (p.rig && p.cells === null) return `rigframe/${rigPath(p, p.pal)}/0`;
  return (
    `preview/${p.arc}:${p.chr}/${p.arc}:${p.pal}?cols=${cols}&paloffset=${p.offset}` +
    (p.cells === undefined ? "" : `&cells=${p.cells ?? "none"}`)
  );
}

/// A name for files saved from the picture: bare numbers get the archive's.
export function pictureName(p: Picture): string {
  return /^(#|Screen)/.test(p.label) ? `${archiveLabel(p.arc)} ${p.label}` : p.label;
}

interface Sequence {
  frames: { cell: number; duration: number }[];
}

// The animation frames as [bridge path, file name] pairs.
async function frames(p: Picture): Promise<[string, string][]> {
  const name = fileName(pictureName(p));
  const num = (i: number, n: number) => String(i + 1).padStart(String(n).length, "0");
  if (p.rig && p.pal !== null) {
    const path = rigPath(p, p.pal);
    const info = await fetch(asset(`riginfo/${path}`)).then((r) => r.json());
    return Array.from({ length: info.frames as number }, (_, i) => [
      `rigframe/${path}/${i}`,
      `${name} frame ${num(i, info.frames)}.png`,
    ]);
  }
  if (p.pal === null) return [];
  const cer = p.cells !== undefined ? p.cells : await cellBank([p.arc, p.chr]).catch(() => null);
  if (cer === null) return [];
  const anr = p.anims !== undefined ? p.anims : await animBank(p.arc, cer).catch(() => null);
  if (anr === null) return [];
  const seqs: Sequence[] = await fetch(asset(`animinfo/${p.arc}:${anr}`)).then((r) => r.json());
  const out: [string, string][] = [];
  const moving = seqs.map((s, i) => [s, i] as const).filter(([s]) => s.frames.length > 1);
  for (const [s, i] of moving) {
    s.frames.forEach((_, n) =>
      out.push([
        `anim/${p.arc}:${p.chr}/${p.arc}:${p.pal}/${p.arc}:${cer}/${p.arc}:${anr}/${i}/${n}?paloffset=${p.offset}`,
        moving.length > 1
          ? `${name} animation ${i + 1} frame ${num(n, s.frames.length)}.png`
          : `${name} frame ${num(n, s.frames.length)}.png`,
      ]),
    );
  }
  return out;
}

// The palette as a JASC-PAL file, which paint programs read.
async function paletteText(p: Picture): Promise<string> {
  const data = await sheetData([p.arc, p.chr], [p.arc, p.pal!], { kind: "grid", cols: 16 });
  const rows = data.colors.map((c) => `${c >> 16} ${(c >> 8) & 255} ${c & 255}`);
  return ["JASC-PAL", "0100", String(rows.length), ...rows, ""].join("\r\n");
}

// What an import lays the PNG over for a picture of the list: the picture as
// its thumbnail shows it (a battle sprite takes its first frame).
async function importTarget(p: Picture): Promise<ImportTarget> {
  const pal: Ref = [p.arc, p.pal!];
  const title = pictureName(p);
  const ref = (m: number): Ref => [p.arc, m];
  if (p.scr !== undefined) {
    return { chr: ref(p.chr), pal, layout: { kind: "screen", scr: ref(p.scr) }, bank: p.offset, title };
  }
  if (p.rig && p.cells === null) {
    const [chr, cer, anr, mcr, mna] = p.rig;
    const layout: Layout = { kind: "rig", cer: ref(cer), anr: ref(anr), mcr: ref(mcr), mna: mna === undefined ? null : ref(mna), frame: 0 };
    return { chr: ref(chr), pal, layout, bank: p.offset, title };
  }
  const cer = p.cells !== undefined ? p.cells : await cellBank(ref(p.chr)).catch(() => null);
  const layout: Layout = cer !== null ? { kind: "cells", cer: ref(cer) } : { kind: "grid", cols: 16 };
  return { chr: ref(p.chr), pal, layout, bank: p.offset, title };
}

/// The items for a picture. `onChanged` runs after a revert or an import.
/// `importAs` is the picture as the open editor lays it out (a battle
/// animation frame when `frame`); the list resolves its own.
export function pictureMenu(
  p: Picture,
  opts: {
    changed: boolean;
    readOnly: boolean;
    onChanged: () => void;
    importAs?: { target: ImportTarget; frame: boolean };
  },
): MenuItem[] {
  const name = pictureName(p);
  const animated = !!p.rig || (p.anims !== undefined && p.anims !== null) || p.anims === undefined;
  return [
    {
      label: "Save picture as PNG",
      disabled: p.pal === null,
      run: guarded(() => savePng(picturePath(p, 16), name)),
    },
    ...(animated
      ? [
          {
            label: "Save animation frames",
            run: guarded(async () => {
              const list = await frames(p);
              if (!list.length) return toast("This picture has no animation.");
              await saveAssets(list);
            }),
          },
        ]
      : []),
    {
      label: "Save palette",
      disabled: p.pal === null,
      run: guarded(async () => saveText(await paletteText(p), `${name} palette`, "pal", "Palette")),
    },
    {
      label: "Save game file",
      run: guarded(() => saveMember(p.arc, p.chr, name, "ncgr")),
    },
    ...(opts.readOnly || p.pal === null
      ? []
      : [
          {
            label: opts.importAs?.frame ? "Import frame..." : "Import PNG...",
            run: guarded(async () =>
              importPng(opts.importAs?.target ?? (await importTarget(p)), opts.onChanged),
            ),
          },
        ]),
    "-",
    { label: "Copy name", run: () => copyText(p.label) },
    {
      label: "Open in Files",
      run: () => openInFiles(p.arc, p.chr),
    },
    ...(opts.changed && !opts.readOnly
      ? [
          "-" as const,
          {
            label: "Revert to game",
            run: guarded(async () => {
              await revert([p.arc, p.chr]);
              toast("Reverted to the game's version");
              opts.onChanged();
            }),
          },
        ]
      : []),
  ];
}

