"""Finds the code tables kenon reads in every Pokemon Black and White cartridge.

kenon reads a few tables straight from the game's code (ARM9 and overlay 10).
Their addresses move between versions and languages. This tool finds each table
in each cartridge's unpacked code and writes core/src/addresses.json.

    python tools/code-addresses.py <aletheia checkout>

It needs aletheia's extracted cartridges (extract/, extract_white/,
extract_black_fr/ and so on: see aletheia docs/ROMS.md). Each table is found by
its English content, or else through the code that points at it (the literal
that loads its address), or else at a fixed distance from a neighbour that was
found. A table that cannot be found is left out for that cartridge, and kenon
does without it.
"""

import json
import os
import struct
import sys

# name, overlay (None: ARM9), English address, length in bytes, neighbour, offset
TABLES = [
    ("type_palette", None, 0x0209FCF8, 18, None, 0),
    ("move_kind_palette", None, 0x0209FCEC, 3, "type_palette", -0xC),
    ("text_speed", None, 0x0209DFB0, 5, None, 0),
    ("trainer_graphic", None, 0x020A023C, 106, None, 0),
    ("item_icons", None, 0x0209EB6C, 627 * 4, None, 0),
    ("icon_palette", None, 0x0209E474, 700, None, 0),
    ("cursor_tile", None, 0x0209E8CC, 32, None, 0),
    ("packed_chars", None, 0x0209E864, 0x32 * 2, None, 0),
    ("timezone_light", None, 0x0209DF10, 20, None, 0),
    ("machine_moves", None, 0x0209EAA0, 202, None, 0),
    ("script_ranges", 10, 0x021701A8, 460, None, 0),
]

# game code: extraction folder in the aletheia checkout
TARGETS = {
    "IRBO": "extract", "IRAO": "extract_white",
    "IRBF": "extract_black_fr", "IRAF": "extract_white_fr",
    "IRBD": "extract_black_de", "IRAD": "extract_white_de",
    "IRBI": "extract_black_it", "IRAI": "extract_white_it",
    "IRBS": "extract_black_es", "IRAS": "extract_white_es",
    "IRBJ": "extract_black_ja", "IRAJ": "extract_white_ja",
}


def images(folder):
    """The ARM9 image and every overlay: {overlay or None: (address, bytes)}."""
    out = {None: (0x02004000, open(os.path.join(folder, "arm9_dec.bin"), "rb").read())}
    y9 = open(os.path.join(folder, "y9.bin"), "rb").read()
    dec = os.path.join(folder, "overlays_dec")
    for name in os.listdir(dec):
        n = int(name[-8:-4])
        out[n] = (struct.unpack_from("<I", y9, n * 32 + 4)[0], open(os.path.join(dec, name), "rb").read())
    return out


def at(imgs, ov, addr, length):
    base, data = imgs[ov]
    return data[addr - base:addr - base + length]


def find_all(data, needle):
    hits, i = [], data.find(needle)
    while i >= 0:
        hits.append(i)
        i = data.find(needle, i + 1)
    return hits


def by_literal(eng, imgs, ov, addr):
    """The address the target loads where English code loads `addr`."""
    found = set()
    lit = struct.pack("<I", addr)
    for base, data in eng.values():
        for i in find_all(data, lit):
            if i % 4 or i < 12:
                continue
            ctx = data[i - 12:i]
            for tbase, tdata in imgs.values():
                for k in find_all(tdata, ctx):
                    found.add(struct.unpack_from("<I", tdata, k + 12)[0])
    found = {a for a in found if imgs[ov][0] <= a < imgs[ov][0] + len(imgs[ov][1])}
    return found.pop() if len(found) == 1 else None


def main():
    root = sys.argv[1]
    eng = images(os.path.join(root, TARGETS["IRBO"]))
    out = {}
    for code, folder in TARGETS.items():
        imgs = images(os.path.join(root, folder))
        found = {}
        for name, ov, addr, length, near, off in TABLES:
            want = at(eng, ov, addr, length)
            base, data = imgs[ov]
            hits = find_all(data, want)
            if len(hits) == 1:
                found[name] = base + hits[0]
                continue
            if near and near in found:
                guess = found[near] + off
                if at(imgs, ov, guess, length) == want:
                    found[name] = guess
                    continue
            guess = by_literal(eng, imgs, ov, addr)
            if guess is not None:
                found[name] = guess
                continue
            # The table's first 40 bytes, when they occur once.
            hits = find_all(data, want[:40]) if length >= 40 else []
            if len(hits) == 1:
                found[name] = base + hits[0]
                continue
            print(f"{code}: {name} not found", file=sys.stderr)
        out[code] = {k: f"0x{v:08X}" for k, v in found.items()}
    path = os.path.join(os.path.dirname(__file__), "..", "core", "src", "addresses.json")
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        json.dump(out, f, indent=1)
        f.write("\n")


if __name__ == "__main__":
    main()
