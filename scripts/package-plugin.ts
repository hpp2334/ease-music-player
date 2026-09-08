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
// `--bundle <id[,id…]>` (repeatable) additionally copies those zips into
// the APK assets (`android/app/src/main/assets/plugin-bundles/<id>.zip`)
// for offline ensure-install by the Rust bootstrap — WebDAV (storage) and
// Lyric Formats (the only lyric parser; the app has no built-in one).
//
// Usage:
//   npx tsx scripts/package-plugin.ts <pluginDir> [...] [--bundle <id[,id…]>]
//   npx tsx scripts/package-plugin.ts --all [--bundle <id[,id…]>]

const REGISTRY_DIR = path.join(ROOT, "plugins/registry");
const ZIPS_DIR = path.join(REGISTRY_DIR, "zips");
const ICONS_DIR = path.join(REGISTRY_DIR, "icons");
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
  /** Registry-served plugin icon (path relative to the registry root);
   *  absent when the plugin ships no icon. */
  icon?: string;
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

  // Plugin-level icon: the zip already carries it (collectIconFiles walks
  // the root key too); also publish it as a standalone registry file the
  // app fetches for not-yet-installed entries.
  if (typeof manifest.icon === "string" && manifest.icon) {
    const iconSrc = path.join(pluginDir, manifest.icon);
    if (!existsSync(iconSrc)) {
      throw new Error(`manifest icon not found: ${iconSrc}`);
    }
    mkdirSync(ICONS_DIR, { recursive: true });
    const iconName = `${id}${path.extname(manifest.icon) || ".png"}`;
    copyFileSync(iconSrc, path.join(ICONS_DIR, iconName));
    entry.icon = `icons/${iconName}`;
    console.log(`registry icon: ${path.relative(ROOT, iconSrc)} -> ${entry.icon}`);
  }

  console.log(`packaged ${id}@${version} -> ${path.relative(ROOT, zipPath)} (${entry.size} bytes)`);
  return entry;
}

const argv = process.argv.slice(2);
const bundleIds = new Set<string>();
for (let i = 0; i < argv.length; ) {
  if (argv[i] === "--bundle") {
    for (const id of (argv[i + 1] ?? "").split(",").map((s) => s.trim()).filter(Boolean)) {
      bundleIds.add(id);
    }
    argv.splice(i, 2);
  } else {
    i += 1;
  }
}

const pluginArgs = argv.filter((a) => !a.startsWith("--"));
const useAll = argv.includes("--all");

const pluginDirs = useAll
  ? readdirSync(path.join(ROOT, "plugins"), { withFileTypes: true })
      .filter((d) => d.isDirectory() && !d.name.startsWith(".") && d.name !== "infra" && d.name !== "registry")
      .map((d) => path.join(ROOT, "plugins", d.name))
      .filter((d) => existsSync(path.join(d, "manifest.json")))
  : pluginArgs.map((a) => path.resolve(ROOT, a));

if (pluginDirs.length === 0) {
  console.error("usage: npx tsx scripts/package-plugin.ts <pluginDir>... | --all [--bundle <id[,id…]>]");
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
  if (bundleIds.has(entry.id)) {
    mkdirSync(ASSET_BUNDLES_DIR, { recursive: true });
    const dest = path.join(ASSET_BUNDLES_DIR, `${entry.id}.zip`);
    copyFileSync(path.join(ZIPS_DIR, path.basename(entry.zip)), dest);
    console.log(`bundled ${entry.id} -> ${path.relative(ROOT, dest)}`);
  }
}
registry.plugins.sort((a, b) => a.id.localeCompare(b.id));
writeFileSync(INDEX_PATH, JSON.stringify(registry, null, 2) + "\n");
console.log(`registry index updated: ${path.relative(ROOT, INDEX_PATH)}`);

// The index is latest-only — prune zips it no longer references so
// superseded versions don't accumulate in the committed registry
// (the fetch flow only ever downloads indexed zips).
const referenced = new Set(registry.plugins.map((p) => path.basename(p.zip)));
for (const file of readdirSync(ZIPS_DIR)) {
  if (!file.endsWith(".zip") || referenced.has(file)) continue;
  execSync(`rm -f ${JSON.stringify(path.join(ZIPS_DIR, file))}`);
  console.log(`pruned stale zip: zips/${file}`);
}
