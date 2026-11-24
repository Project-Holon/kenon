"""Writes core/src/code_names.tsv.gz: aletheia's function names and where
they sit in each cartridge, so kenon can name the game code a ROM hack
changes.

    python tools/code-names.py <aletheia checkout>

It reads two of aletheia's layout files (names, addresses and sizes only, no
game bytes):

- layout/function_addrs.tsv: every function of English Black (IRBO) with its
  region, address, size, instruction set and source file;
- layout/langmap/<target>.tsv: runs of English addresses and where they sit in
  another cartridge. Only a region's own runs are kept (owner = region).

Output lines, tab separated:

    F  region  address  size  t|a  name  file     (a function, English address)
    M  game    region   english_start  english_end  target_start

The Japanese cartridges have no map, so kenon names nothing there.
"""

import gzip
import os
import sys

# game code: langmap target (English Black is the table itself)
TARGETS = {
    "IRAO": "white",
    "IRBF": "black_fr", "IRAF": "white_fr",
    "IRBD": "black_de", "IRAD": "white_de",
    "IRBI": "black_it", "IRAI": "white_it",
    "IRBS": "black_es", "IRAS": "white_es",
}


def region_name(r):
    """langmap calls the ARM9 static module arm9_main; the function table arm9."""
    return "arm9" if r == "arm9_main" else r


def main():
    root = sys.argv[1]
    out = []
    with open(os.path.join(root, "layout", "function_addrs.tsv"), encoding="utf-8") as f:
        next(f)
        for line in f:
            region, addr, name, isa, size, path, _static = line.rstrip("\n").split("\t")
            path = path.removeprefix("src/game/")
            out.append(f"F\t{region}\t{int(addr, 16):08X}\t{int(size or 0)}\t{isa[0]}\t{name}\t{path}")
    for game, target in TARGETS.items():
        with open(os.path.join(root, "layout", "langmap", f"{target}.tsv"), encoding="utf-8") as f:
            next(f)
            for line in f:
                region, start, end, tstart, owner = line.rstrip("\n").split("\t")
                if owner != region:
                    continue
                out.append(
                    f"M\t{game}\t{region_name(region)}\t{int(start, 16):08X}\t{int(end, 16):08X}\t{int(tstart, 16):08X}"
                )
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "core", "src", "code_names.tsv.gz")
    data = ("\n".join(out) + "\n").encode("utf-8")
    # mtime 0 and no file name: the same input writes the same bytes
    with open(path, "wb") as raw, gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0, compresslevel=9) as g:
        g.write(data)
    print(f"{sum(1 for l in out if l[0] == 'F')} functions, {sum(1 for l in out if l[0] == 'M')} map runs, "
          f"{len(data)} bytes, {os.path.getsize(path)} packed")


if __name__ == "__main__":
    main()
