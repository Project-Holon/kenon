"""Builds core/src/reference.json.gz: Black and White's own text in English,
French, German, Italian and Spanish for the banks a mod most often
translates, from PokeAPI's data of the games (version group black-white).
kenon shows it beside a translation when no ROM of that language is set.

    python -I tools/make_reference.py <pokeapi csv folder> <rom_en_names.tsv>

rom_en_names.tsv (bank, line, English text) maps items: PokeAPI numbers
items its own way, so they are matched by their English name in the ROM.
"""
import csv
import gzip
import json
import sys
from pathlib import Path

api = Path(sys.argv[1])
rom = Path(sys.argv[2])
LANGS = {"en": "9", "fr": "5", "de": "6", "it": "8", "es": "7"}
BW = "11"
# Later text where PokeAPI has none of Black and White's in a language
# (German, Italian and Spanish descriptions): X and Y's.
XY = "15"
VERSION = {"17": 236, "18": 235}  # Black, White Pokedex entries
XY_VERSION = {"23": 236, "24": 235}  # X, Y: the same slots
# Item names the games changed since Black and White, BW's to PokeAPI's.
ITEM_ALIASES = {"parlyzheal": "paralyzeheal", "xdefend": "xdefense", "xspecial": "xspatk",
                "dowsingmchn": "dowsingmachine", "healthwing": "healthfeather", "musclewing": "musclefeather",
                "resistwing": "resistfeather", "geniuswing": "geniusfeather", "cleverwing": "cleverfeather",
                "swiftwing": "swiftfeather", "prettywing": "prettyfeather"}
LAST = {"species": 649, "move": 559, "ability": 164, "type": 17}


def rows(name):
    with open(api / f"{name}.csv", encoding="utf-8") as f:
        yield from csv.DictReader(f)


def clean(text):
    # The games' line breaks become the text form's \n; soft hyphens at a
    # break become hyphens.
    t = text.replace("­\n", "-\n").replace("­", "-").replace("\f", "\n").replace("\r", "")
    return "\\n".join(part.strip() for part in t.split("\n"))


lang_of = {v: k for k, v in LANGS.items()}
out = {lang: {} for lang in LANGS}
later = {lang: set() for lang in LANGS}


def put(lang, bank, line, text, from_later=False):
    # The games' plain apostrophe, as the cartridges have it.
    text = text.replace("’", "'")
    lines = out[lang].setdefault(str(bank), {})
    if from_later and str(line) in lines:
        return
    lines[str(line)] = text
    if from_later:
        later[lang].add(str(bank))


def key(name):
    k = "".join(c for c in name.lower() if c.isalnum())
    return ITEM_ALIASES.get(k, k)


for r in rows("pokemon_species_names"):
    s, lang = int(r["pokemon_species_id"]), lang_of.get(r["local_language_id"])
    if lang and s <= LAST["species"]:
        put(lang, 70, s, r["name"])
        if r["genus"]:
            put(lang, 260, s, r["genus"])
for r in rows("pokemon_species_flavor_text"):
    s, lang = int(r["species_id"]), lang_of.get(r["language_id"])
    if lang and s <= LAST["species"] and r["version_id"] in VERSION:
        put(lang, VERSION[r["version_id"]], s, clean(r["flavor_text"]))
for r in rows("pokemon_species_flavor_text"):
    s, lang = int(r["species_id"]), lang_of.get(r["language_id"])
    if lang and s <= LAST["species"] and r["version_id"] in XY_VERSION:
        put(lang, XY_VERSION[r["version_id"]], s, clean(r["flavor_text"]), True)
for r in rows("move_names"):
    m, lang = int(r["move_id"]), lang_of.get(r["local_language_id"])
    if lang and m <= LAST["move"]:
        put(lang, 203, m, r["name"])
for r in rows("move_flavor_text"):
    m, lang = int(r["move_id"]), lang_of.get(r["language_id"])
    if lang and m <= LAST["move"] and r["version_group_id"] == BW:
        put(lang, 202, m, clean(r["flavor_text"]))
for r in rows("move_flavor_text"):
    m, lang = int(r["move_id"]), lang_of.get(r["language_id"])
    if lang and m <= LAST["move"] and r["version_group_id"] == XY:
        put(lang, 202, m, clean(r["flavor_text"]), True)
for r in rows("ability_names"):
    a, lang = int(r["ability_id"]), lang_of.get(r["local_language_id"])
    if lang and a <= LAST["ability"]:
        put(lang, 182, a, r["name"])
for r in rows("ability_flavor_text"):
    a, lang = int(r["ability_id"]), lang_of.get(r["language_id"])
    if lang and a <= LAST["ability"] and r["version_group_id"] == BW:
        put(lang, 183, a, clean(r["flavor_text"]))
for r in rows("ability_flavor_text"):
    a, lang = int(r["ability_id"]), lang_of.get(r["language_id"])
    if lang and a <= LAST["ability"] and r["version_group_id"] == XY:
        put(lang, 183, a, clean(r["flavor_text"]), True)
for r in rows("type_names"):
    t, lang = int(r["type_id"]), lang_of.get(r["local_language_id"])
    if lang and t <= LAST["type"]:
        put(lang, 199, t - 1, r["name"])

# Items: the ROM's English names to PokeAPI's item ids.
rom_items = {}
for line in rom.read_text(encoding="utf-8").splitlines():
    bank, idx, text = line.split("\t", 2)
    if bank == "54" and text and text not in ("???",):
        rom_items.setdefault(key(text), []).append(int(idx))
api_en = {r["item_id"]: r["name"] for r in rows("item_names") if r["local_language_id"] == "9"}
game_of = {}
for api_id, name in api_en.items():
    for idx in rom_items.get(key(name), []):
        game_of[api_id] = idx
for r in rows("item_names"):
    lang = lang_of.get(r["local_language_id"])
    if lang and r["item_id"] in game_of:
        put(lang, 54, game_of[r["item_id"]], r["name"])
for r in rows("item_flavor_text"):
    lang = lang_of.get(r["language_id"])
    if lang and r["item_id"] in game_of and r["version_group_id"] == BW:
        put(lang, 53, game_of[r["item_id"]], clean(r["flavor_text"]))
for r in rows("item_flavor_text"):
    lang = lang_of.get(r["language_id"])
    if lang and r["item_id"] in game_of and r["version_group_id"] == XY:
        put(lang, 53, game_of[r["item_id"]], clean(r["flavor_text"]), True)
matched = set(game_of.values())
unmatched = sorted({i for v in rom_items.values() for i in v} - matched)

# Every official name of later games too, by its English name, so a mod's
# new move, item, ability, type or Pokemon named as in a later game fills in
# every language (kenon's Fill official names).
names = {"move": {}, "item": {}, "ability": {}, "type": {}, "species": {}}
SOURCES = [("move", "move_names", "move_id", "name"), ("item", "item_names", "item_id", "name"),
           ("ability", "ability_names", "ability_id", "name"), ("type", "type_names", "type_id", "name"),
           ("species", "pokemon_species_names", "pokemon_species_id", "name")]
for kind, table, idcol, col in SOURCES:
    by_id = {}
    for r in rows(table):
        lang = lang_of.get(r["local_language_id"])
        if lang and r[col]:
            by_id.setdefault(r[idcol], {})[lang] = r[col].replace("’", "'")
    for row in by_id.values():
        en = row.get("en")
        if en:
            names[kind][en] = {l: n for l, n in row.items() if l != "en"}

data = {
    "about": "Black and White's text from PokeAPI (pokeapi.co), by bank and line; banks in later are X and Y's where PokeAPI has none of Black and White's.",
    "languages": out,
    "names": names,
    "later": {lang: sorted(b, key=int) for lang, b in later.items()},
}
dest = Path(__file__).resolve().parent.parent / "core/src/reference.json.gz"
with gzip.open(dest, "wt", encoding="utf-8", compresslevel=9) as f:
    json.dump(data, f, ensure_ascii=False, separators=(",", ":"))
print(dest, dest.stat().st_size, "bytes")
for lang, banks in out.items():
    print(lang, {b: len(l) for b, l in sorted(banks.items(), key=lambda x: int(x[0]))})
print("official names", {k: len(v) for k, v in names.items()})
print("items matched", len(matched), "unmatched", len(unmatched), unmatched[:40])
