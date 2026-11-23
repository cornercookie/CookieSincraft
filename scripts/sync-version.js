//! Version synchronization script for CookieSincraft.
//!
//! Reads `src-tauri/Cargo.toml` `version` as the authoritative source and
//! synchronises it to `src-tauri/tauri.conf.json` and `src/package.json`.
//!
//! Usage:
//!   node scripts/sync-version.js          # write the Cargo.toml version to the other two files
//!   node scripts/sync-version.js --check  # exit 0 if all three match, non-zero otherwise

const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..");
const CARGO_TOML = path.join(ROOT, "src-tauri", "Cargo.toml");
const TAURI_CONF = path.join(ROOT, "src-tauri", "tauri.conf.json");
const PACKAGE_JSON = path.join(ROOT, "src", "package.json");

function readCargoVersion() {
    const content = fs.readFileSync(CARGO_TOML, "utf8");
    const match = content.match(/^version\s*=\s*"([^"]+)"/m);
    if (!match) {
        console.error(`ERROR: Could not parse version from ${CARGO_TOML}`);
        process.exit(1);
    }
    return match[1];
}

function readJsonVersion(filePath) {
    const content = JSON.parse(fs.readFileSync(filePath, "utf8"));
    return content.version;
}

function writeJsonVersion(filePath, version) {
    const content = JSON.parse(fs.readFileSync(filePath, "utf8"));
    content.version = version;
    fs.writeFileSync(filePath, JSON.stringify(content, null, 2) + "\n");
}

function main() {
    const cargoVersion = readCargoVersion();
    const args = process.argv.slice(2);

    if (args.includes("--check")) {
        const tauriVersion = readJsonVersion(TAURI_CONF);
        const npmVersion = readJsonVersion(PACKAGE_JSON);
        const ok = cargoVersion === tauriVersion && tauriVersion === npmVersion;

        if (ok) {
            console.log(`OK: All three version fields are ${cargoVersion}`);
            process.exit(0);
        } else {
            console.error(
                `ERROR: Version mismatch:\n  Cargo.toml: ${cargoVersion}\n  tauri.conf.json: ${tauriVersion}\n  package.json: ${npmVersion}`
            );
            process.exit(1);
        }
    } else {
        writeJsonVersion(TAURI_CONF, cargoVersion);
        writeJsonVersion(PACKAGE_JSON, cargoVersion);
        console.log(`Synced version to ${cargoVersion}`);
    }
}

main();
