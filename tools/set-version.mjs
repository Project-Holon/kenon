// Sets the app version everywhere it is written: the Cargo workspace,
// package.json and the Tauri config. Usage: node tools/set-version.mjs 0.2.0
// With no version, checks that the three agree (CI runs it that way).
import { readFileSync, writeFileSync } from "node:fs";

const files = {
  cargo: "Cargo.toml",
  npm: "package.json",
  tauri: "app/tauri.conf.json",
};

const read = (f) => readFileSync(f, "utf8");
const cargoVersion = (s) => s.match(/\[workspace\.package\][^[]*?\nversion = "([^"]+)"/)?.[1];
const jsonVersion = (s) => JSON.parse(s).version;

const wanted = process.argv[2];
if (!wanted) {
  const found = {
    [files.cargo]: cargoVersion(read(files.cargo)),
    [files.npm]: jsonVersion(read(files.npm)),
    [files.tauri]: jsonVersion(read(files.tauri)),
  };
  const values = new Set(Object.values(found));
  if (values.size !== 1) {
    console.error("versions differ:", found);
    process.exit(1);
  }
  console.log([...values][0]);
  process.exit(0);
}

if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(wanted)) {
  console.error(`not a version: ${wanted} (use major.minor.patch, like 0.2.0)`);
  process.exit(1);
}
const cargo = read(files.cargo).replace(
  /(\[workspace\.package\][^[]*?\nversion = ")[^"]+(")/,
  `$1${wanted}$2`,
);
writeFileSync(files.cargo, cargo);
for (const f of [files.npm, files.tauri]) {
  writeFileSync(f, read(f).replace(/("version":\s*")[^"]+(")/, `$1${wanted}$2`));
}
console.log(`version ${wanted}: run cargo check to update Cargo.lock, commit, then tag v${wanted}`);
