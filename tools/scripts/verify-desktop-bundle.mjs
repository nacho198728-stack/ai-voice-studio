import { execFileSync } from "node:child_process";
import { stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const VALID_TARGET = /^[A-Za-z0-9_-]{1,128}$/;

export async function verifyDesktopBundle(bundleRoot, target) {
  if (!VALID_TARGET.test(target)) {
    throw new Error("Rust target triple must be bounded ASCII without traversal");
  }
  const windows = target.includes("windows");
  const apple = target.includes("apple-darwin");
  const prefix = windows ? "" : "Contents/";
  const executableDirectory = windows ? "" : `${prefix}MacOS/`;
  const resourceDirectory = windows ? "" : `${prefix}Resources/`;
  const expected = {
    application: path.join(
      bundleRoot,
      executableDirectory,
      windows ? "ai-voice-studio.exe" : "ai-voice-studio",
    ),
    runtime: path.join(
      bundleRoot,
      executableDirectory,
      windows ? "voice-runtime.exe" : "voice-runtime",
    ),
    plugin: path.join(
      bundleRoot,
      resourceDirectory,
      "native",
      `aivs_mock_voice_engine-${target}.${windows ? "dll" : apple ? "dylib" : "so"}`,
    ),
    config: path.join(bundleRoot, resourceDirectory, "config", "config.json"),
  };
  for (const candidate of Object.values(expected)) {
    if (!(await isFile(candidate))) {
      throw new Error("desktop bundle is missing a required Tauri-owned artifact");
    }
  }
  return expected;
}

async function isFile(candidate) {
  try {
    return (await stat(candidate)).isFile();
  } catch {
    return false;
  }
}

function hostTarget() {
  const output = execFileSync("rustc", ["-vV"], { encoding: "utf8" });
  const host = output
    .split(/\r?\n/u)
    .find((line) => line.startsWith("host: "))
    ?.slice("host: ".length);
  if (!host || !VALID_TARGET.test(host)) throw new Error("rustc did not report a valid host target");
  return host;
}

const scriptPath = fileURLToPath(import.meta.url);
if (process.argv[1] && path.resolve(process.argv[1]) === scriptPath) {
  const [bundleRoot, target = hostTarget()] = process.argv.slice(2);
  if (!bundleRoot) {
    process.stderr.write("usage: verify-desktop-bundle.mjs <bundle-root> [target]\n");
    process.exitCode = 2;
  } else {
    try {
      const verified = await verifyDesktopBundle(path.resolve(bundleRoot), target);
      process.stdout.write(`Verified desktop bundle artifacts:\n${Object.values(verified).join("\n")}\n`);
    } catch (error) {
      process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
      process.exitCode = 1;
    }
  }
}
