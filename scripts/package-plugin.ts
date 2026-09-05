import { execSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync, copyFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { ROOT } from "./base";

// Package built plugin bundles (rspack output in <pluginDir>/dist) into
// the committed registry:
//
//   plugins/registry/zips/<id>-<version>.zip   (manifest.json + *.js at zip root)
//   plugins/registry/plugins.json              (index: id/name/version/zip/sha256/size)
//
// `--bundle <id>` additionally copies the zip into the APK assets
// (`android/app/src/main/assets/plugin-bundles/<id>.zip`) for first-run
// offline install (only WebDAV ships there).
//
// Usage:
//   npx tsx scripts/package-plugin.ts <pluginDir> [...] [--bundle <id>]
//   npx tsx scripts/package-plugin.ts --all [--bundle <id>]

const REGISTRY_DIR = path.join(ROOT, "plugins/registry");
const ZIPS_DIR = path.join(REGISTRY_DIR, "zips");
const INDEX_PATH = path.join(REGISTRY_DIR, "plugins.json");
const ASSET_BUNDLES_DIR = path.join(ROOT, "android/app/src/main/assets/plugin-bundles");

interface RegistryEntry {
  id: string;
  name: string | Record<string, string>;
  version: string;
  description: string | Record<string, string>;
  zip: string;
  sha256: string;
  size: number;
}

interface Registry {
  plugins: RegistryEntry[];
}

function readRegistry(): Registry {
  if (!existsSync(INDEX_PATH)) return { plugins: [] };
  try {
    return JSON.parse(readFileSync(INDEX_PATH, "utf8"));
  } catch {
    return { plugins: [] };
  }
}

function sha256(file: string): string {
  return createHash("sha256").update(readFileSync(file)).digest("hex");
}

/** Collect every `"icon": "<file>"` value in the parsed manifest (deep). */
function collectIconFiles(node: unknown, out: Set<string> = new Set()): Set<string> {
  if (Array.isArray(node)) {
    for (const item of node) collectIconFiles(item, out);
  } else if (node && typeof node === "object") {
    for (const [key, value] of Object.entries(node)) {
      if (key === "icon" && typeof value === "string" && value) {
        out.add(value);
      } else {
        collectIconFiles(value, out);
      }
    }
  }
  return out;
}

function packagePlugin(pluginDir: string): RegistryEntry {
  const manifestPath = path.join(pluginDir, "manifest.json");
  if (!existsSync(manifestPath)) {
    throw new Error(`manifest.json not found: ${manifestPath}`);
  }
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  const id: string = manifest.id;
  const version: string = manifest.version ?? "0.0.0";
  const distDir = path.join(pluginDir, "dist");
  if (!existsSync(distDir)) {
    throw new Error(`dist/ not found — run the plugin's rspack build first: ${distDir}`);
  }
  // The manifest ships inside the zip root (the old per-plugin `cp` step
  // only targeted the removed assets dir).
  copyFileSync(manifestPath, path.join(distDir, "manifest.json"));

  // Contribution icons are plain files (never bundled by rspack) — copy
  // each manifest-referenced one into dist/ so it lands in the zip root.
  // A missing icon is a packaging error: the runtime would silently drop
  // the icon, so fail loudly here instead.
  for (const icon of collectIconFiles(manifest)) {
    const src = path.join(pluginDir, icon);
    if (!existsSync(src)) {
      throw new Error(`manifest icon not found: ${src}`);
    }
    copyFileSync(src, path.join(distDir, icon));
    console.log(`icon: ${icon} -> ${path.join(distDir, icon)}`);
  }

  mkdirSync(ZIPS_DIR, { recursive: true });
  const zipPath = path.join(ZIPS_DIR, `${id}-${version}.zip`);
  // Zip the dist *contents* (manifest + icon(s) + bundles at zip root),
  // stored-only entries (-X drops extra attrs; -0 avoids double compression
  // of already-minified JS and keeps installs fast on-device). Source maps
  // stay out — they double the payload for no runtime value. The zip is
  // recreated from scratch (`zip` would otherwise merge into a stale
  // archive).
  execSync(`rm -f "${zipPath}" && zip -rX -0 "${zipPath}" . -x "*.map"`, { cwd: distDir, stdio: "inherit" });

  // `name` / `description` may be localized (`{ "en-US": …, "zh-CN": … }`) —
  // pass them through as declared; the app normalizes on parse.
  const entry: RegistryEntry = {
    id,
    name: manifest.name ?? id,
    version,
    description: manifest.description ?? "",
    zip: `zips/${path.basename(zipPath)}`,
    sha256: sha256(zipPath),
    size: statSync(zipPath).size,
  };
  console.log(`packaged ${id}@${version} -> ${path.relative(ROOT, zipPath)} (${entry.size} bytes)`);
  return entry;
}

const argv = process.argv.slice(2);
const bundleFlagIdx = argv.indexOf("--bundle");
const bundleId = bundleFlagIdx >= 0 ? argv.splice(bundleFlagIdx, 2)[1] : undefined;

const pluginArgs = argv.filter((a) => !a.startsWith("--"));
const useAll = argv.includes("--all");

const pluginDirs = useAll
  ? readdirSync(path.join(ROOT, "plugins"), { withFileTypes: true })
      .filter((d) => d.isDirectory() && !d.name.startsWith(".") && d.name !== "infra" && d.name !== "registry")
      .map((d) => path.join(ROOT, "plugins", d.name))
      .filter((d) => existsSync(path.join(d, "manifest.json")))
  : pluginArgs.map((a) => path.resolve(ROOT, a));

if (pluginDirs.length === 0) {
  console.error("usage: npx tsx scripts/package-plugin.ts <pluginDir>... | --all [--bundle <id>]");
  process.exit(1);
}

const registry = readRegistry();
for (const dir of pluginDirs) {
  const entry = packagePlugin(dir);
  const existing = registry.plugins.find((p) => p.id === entry.id);
  if (existing) {
    Object.assign(existing, entry);
  } else {
    registry.plugins.push(entry);
  }
  if (bundleId === entry.id) {
    mkdirSync(ASSET_BUNDLES_DIR, { recursive: true });
    const dest = path.join(ASSET_BUNDLES_DIR, `${entry.id}.zip`);
    copyFileSync(path.join(ZIPS_DIR, path.basename(entry.zip)), dest);
    console.log(`bundled ${entry.id} -> ${path.relative(ROOT, dest)}`);
  }
}
registry.plugins.sort((a, b) => a.id.localeCompare(b.id));
writeFileSync(INDEX_PATH, JSON.stringify(registry, null, 2) + "\n");
console.log(`registry index updated: ${path.relative(ROOT, INDEX_PATH)}`);
