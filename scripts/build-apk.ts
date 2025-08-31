import { execSync } from "child_process";
import {
  writeFileSync,
  rmSync,
  mkdirSync,
  cpSync,
  readFileSync,
  renameSync,
} from "fs";
import path from "path";
import { BUILD_GRADLE_KTS, ROOT, TARGETS } from "./base";
import fs from "node:fs";
import zlib from "node:zlib";

function decodeAndDecompress(
  base64Encoded: string,
  outputFilePath: string,
): void {
  const decodedBuffer = Buffer.from(base64Encoded, "base64");
  const decompressed = zlib.brotliDecompressSync(decodedBuffer);
  fs.writeFileSync(outputFilePath, decompressed);
}

const { ANDROID_SIGN_PASSWORD, ANDROID_SIGN_JKS } = process.env;

const version = (() => {
  const buildGradleKts = readFileSync(BUILD_GRADLE_KTS, "utf8");
  const versionRegex = /versionName\s*=\s*['"]([^'"]+)['"]/;
  const match = buildGradleKts.match(versionRegex);
  if (!match) {
    throw Error("Failed to extract version from build.gradle.kts");
  }
  return match[1];
})();
console.log(`App version: ${version}`);

const androidDir = path.resolve(ROOT, "./android");
const jksPath = path.resolve(androidDir, "root.jks");
const keyPropertiesPath = path.resolve(androidDir, "key.properties");
const srcDir = path.resolve(androidDir, "./app/build/outputs/apk/release");
const dstDir = path.resolve(ROOT, "./artifacts/apk");

// Generate jks from environment
decodeAndDecompress(ANDROID_SIGN_JKS!, jksPath);

// Signing
writeFileSync(
  keyPropertiesPath,
  `storePassword=${ANDROID_SIGN_PASSWORD}
    keyPassword=${ANDROID_SIGN_PASSWORD}
    keyAlias=key0
    storeFile=root.jks`,
);
console.log(`${keyPropertiesPath} written`);

execSync("./gradlew assembleRelease", {
  stdio: "inherit",
  cwd: androidDir,
});

rmSync(dstDir, { recursive: true, force: true });
mkdirSync(srcDir, { recursive: true });
console.log(srcDir);
cpSync(srcDir, dstDir, { recursive: true });

// rename
for (const target of TARGETS) {
  renameSync(
    path.join(dstDir, `app-${target}-release.apk`),
    path.join(dstDir, `ease-client-music-${target}-${version}.apk`),
  );
}
