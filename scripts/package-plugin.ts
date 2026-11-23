import { execSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync, copyFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { ROOT } from "./base";

// Package rut plugin builds into the committed registry:
//
//   plugins/registry/zips/<id>-<version>.zip   (manifest.json + dist/*.zip + icons)
//   plugins/registry/plugins.json              (index: id/name/version/zip/sha256/size/apiVersion)
//
// Plugins are rut pkgs since the apiVersion 3 epoch. The registry zip
// carries the PACKED artifacts (phase 6): `ease-plugin-packer` (run by
// `pnpm build:plugins` before this script) packs each entrypoint pkg
// (`backend/`, `view/` — rut.jsonc + src/ trees) into `dist/<entry>.zip`
// source-zip artifacts via rut's source-mode pack; this script zips
// `manifest.json + dist/*.zip + icons` — the registry zip preserves
// relative paths so the installed folder matches the source tree.
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

/** The current plugin API epoch — must match `PLUGIN_API_MIN/MAX` in
 *  `rust-libs/ease-client-backend/src/services/plugin_manager.rs` (the app
 *  refuses installs outside its range, so packaging an out-of-epoch zip is
 *  a loud error). 3 = the emp epoch (pkg trees + the `emp` SDK; apiVersion
 *  2 = the retired single-file rut epoch). */
const PLUGIN_API_VERSION = 3;

interface RegistryEntry {
  id: string;
  name: string | Record<string, string>;
  version: string;
  /** Plugin API level the plugin targets (a plain integer, required —
   *  mirrors the manifest's `apiVersion`; the app refuses to install
   *  outside its supported range). */
  apiVersion: number;
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

/** Every manifest entrypoint reference (`backend` + contribution `view`
 *  values) — the dist artifact names the packer produced. */
function collectEntryNames(manifest: {
  backend?: unknown;
  contributions?: Record<string, { view?: unknown }[] | undefined>;
}): string[] {
  const out: string[] = [];
  if (typeof manifest.backend === "string" && manifest.backend) out.push(manifest.backend);
  for (const kind of ["storages", "dashboard"] as const) {
    for (const c of manifest.contributions?.[kind] ?? []) {
      if (typeof c.view === "string" && c.view) out.push(c.view);
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
  // The app rejects installs whose manifest lacks an integer `apiVersion`
  // or whose level is outside its supported range (see PLUGIN_API_MIN/MAX
  // in `plugin_manager.rs`) — fail the packaging loudly instead of shipping
  // an uninstallable zip.
  if (!Number.isInteger(manifest.apiVersion)) {
    throw new Error(
      `manifest.json must declare an integer "apiVersion" (e.g. ${PLUGIN_API_VERSION}): ${manifestPath}`,
    );
  }
  const apiVersion: number = manifest.apiVersion;
  if (apiVersion !== PLUGIN_API_VERSION) {
    throw new Error(
      `manifest apiVersion ${apiVersion} != the packaging epoch ${PLUGIN_API_VERSION} (the emp epoch) — port the plugin first: ${manifestPath}`,
    );
  }

  // Stage the zip contents: manifest + the PACKED entrypoint artifacts
  // (`dist/<entry>.zip` — written by `ease-plugin-packer`, which
  // `pnpm build:plugins` runs before this script) + every
  // manifest-referenced icon. A missing artifact is a packaging error —
  // the runtime would load a blank view / no backend.
  const distDir = path.join(pluginDir, "dist");
  const staging = path.join(distDir, ".stage");
  execSync(`rm -rf "${staging}" && mkdir -p "${staging}/dist"`);
  copyFileSync(manifestPath, path.join(staging, "manifest.json"));

  const entryNames = collectEntryNames(manifest);
  if (entryNames.length === 0) {
    throw new Error(`manifest names no entrypoints: ${manifestPath}`);
  }
  for (const entry of entryNames.sort()) {
    if (!entry.endsWith(".zip")) {
      throw new Error(
        `manifest entry "${entry}" is not a dist pointer — name the packed artifact (e.g. backend.zip): ${manifestPath}`,
      );
    }
    const artifact = path.join(distDir, entry);
    if (!existsSync(artifact)) {
      throw new Error(
        `packed artifact not found: ${artifact} — run the packer first (pnpm build:plugins)`,
      );
    }
    copyFileSync(artifact, path.join(staging, "dist", entry));
    console.log(`artifact: dist/${entry} -> staging`);
  }

  for (const icon of collectIconFiles(manifest)) {
    const src = path.join(pluginDir, icon);
    if (!existsSync(src)) {
      throw new Error(`manifest icon not found: ${src}`);
    }
    copyFileSync(src, path.join(staging, icon));
    console.log(`icon: ${icon} -> ${path.join(staging, icon)}`);
  }

  mkdirSync(ZIPS_DIR, { recursive: true });
  const zipPath = path.join(ZIPS_DIR, `${id}-${version}.zip`);
  // Zip the staged contents (manifest + icons at zip root, the packed
  // artifacts under `dist/`), stored-only (-X drops extra attrs; -0
  // keeps installs fast on-device). Recreated from scratch each run.
  execSync(`rm -f "${zipPath}" && zip -rX -0 "${zipPath}" .`, { cwd: staging, stdio: "inherit" });

  // `name` / `description` may be localized (`{ "en-US": …, "zh-CN": … }`) —
  // pass them through as declared; the app normalizes on parse.
  const entry: RegistryEntry = {
    id,
    name: manifest.name ?? id,
    version,
    apiVersion,
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
