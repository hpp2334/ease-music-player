import { spawn, execSync } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";
import path from "node:path";
import { ROOT, RUST_LIBS_ROOTS } from "./base";

const env = { ...process.env };

const cargoBin = path.join(
  process.env.USERPROFILE || process.env.HOME || "",
  ".cargo",
  "bin",
);
if (existsSync(cargoBin) && !(env.PATH ?? "").includes(cargoBin)) {
  env.PATH = `${cargoBin}${path.delimiter}${env.PATH ?? ""}`;
}

function findJavaHome(): string | undefined {
  if (env.JAVA_HOME && existsSync(path.join(env.JAVA_HOME, "bin"))) {
    return env.JAVA_HOME;
  }
  if (process.platform === "win32") {
    const roots = [
      "C:\\Program Files\\Eclipse Adoptium",
      "C:\\Program Files\\Microsoft",
    ];
    for (const dir of roots) {
      if (!existsSync(dir)) continue;
      const hit = readdirSync(dir)
        .filter((n) => /jdk[-_]?2[0-9]/i.test(n))
        .map((n) => path.join(dir, n))
        .sort()
        .reverse()
        .find((j) => existsSync(path.join(j, "bin", "java.exe")));
      if (hit) return hit;
    }
  }
  return undefined;
}

const javaHome = findJavaHome();
if (!javaHome) {
  console.error(
    "ERROR: JDK not found. Set JAVA_HOME or install Temurin 21 (winget install EclipseAdoptium.Temurin.21.JDK).",
  );
  process.exit(1);
}
env.JAVA_HOME = javaHome;
if (process.platform === "win32") {
  env.PATH = `${path.join(javaHome, "bin")}${path.delimiter}${env.PATH ?? ""}`;
}

console.log(`JAVA_HOME = ${javaHome}`);
console.log("Build ease-client-backend (debug)...");
execSync("cargo build -p ease-client-backend", {
  stdio: "inherit",
  cwd: RUST_LIBS_ROOTS,
  env,
});

const gradlew = process.platform === "win32" ? "gradlew.bat" : "./gradlew";
console.log("Run desktop app: gradlew :composeApp:run");
const child = spawn(gradlew, [":composeApp:run"], {
  stdio: "inherit",
  cwd: ROOT,
  env,
  shell: true,
});

const forward = (sig: NodeJS.Signals) => () => child.kill(sig);
process.on("SIGINT", forward("SIGINT"));
process.on("SIGTERM", forward("SIGTERM"));

child.on("exit", (code) => process.exit(code ?? 0));
