// What the core knows of each archive: a plain label, a group and a note
// (core/src/archives.json).
import data from "../../../core/src/archives.json";

interface Known {
  id: number;
  label: string;
  group: string;
  note?: string;
}

const BY_ID = new Map<number, Known>((data.archives as Known[]).map((a) => [a.id, a]));

export const archiveLabel = (id: number) => BY_ID.get(id)?.label ?? `Archive ${id}`;
export const archiveGroup = (id: number) => BY_ID.get(id)?.group ?? "Other";
export const archiveNote = (id: number) => BY_ID.get(id)?.note ?? "";

// Member formats in plain words; the code stays as a detail.
const KINDS: Record<string, string> = {
  NCGR: "Picture tiles",
  NCLR: "Colours",
  NSCR: "Screen layout",
  NCER: "Sprite cells",
  NANR: "Animation",
  NFTR: "Font",
  NMCR: "Multi-cell sprite",
  NMAR: "Multi-cell animation",
  NSBMD: "3D model",
  NSBTX: "3D textures",
  NSBCA: "Model animation",
  NSBTA: "Model animation",
  NSBMA: "Model animation",
  NSBTP: "Model animation",
  NSBVA: "Model animation",
  EMPTY: "Empty",
  RAW: "Data",
  text: "Text",
  data: "Data",
};
export const kindName = (kind: string) => KINDS[kind] ?? kind;
