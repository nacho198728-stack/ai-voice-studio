import { copyFile, mkdir, stat } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const VALID_TARGET = /^[A-Za-z0-9_-]{1,128}$/;

export function nativeArtifactPlan(profile, target) {
  if (profile !== "Debug" && profile !== "Release") {
    throw new Error("profile must be exactly Debug or Release");
  }
  if (!VALID_TARGET.test(target)) {
    throw new Error("Rust target triple must be bounded ASCII without traversal");
  }

  const preset = profile === "Debug" ? "native-debug" : "native-release";
  const windows = target.includes("windows");
  const apple = target.includes("apple-darwin");
  const runtimeName = windows ? "voice-runtime.exe" : "voice-runtime";
  const pluginSource = windows
    ? `build/${preset}/bin/${profile}/aivs_mock_voice_engine.dll`
    : `build/${preset}/lib/${profile}/libaivs_mock_voice_engine.${apple ? "dylib" : "so"}`;
  const pluginExtension = windows ? "dll" : apple ? "dylib" : "so";

  return {
    runtimeSource: `build/${preset}/bin/${profile}/${runtimeName}`,
    pluginSource,
    runtimeDestination: `apps/desktop/src-tauri/binaries/voice-runtime-${target}${windows ? ".exe" : ""}`,
    pluginDestination:
      `apps/desktop/src-tauri/resources/native/aivs_mock_voice_engine-${target}.${pluginExtension}`,
    configDestination: "apps/desktop/src-tauri/resources/config/config.json",
  };
}

export async function stageNativeArtifacts({ repositoryRoot, profile, target }) {
  const plan = nativeArtifactPlan(profile, target);
  const sources = {
    runtime: path.join(repositoryRoot, plan.runtimeSource),
    plugin: path.join(repositoryRoot, plan.pluginSource),
    config: path.join(repositoryRoot, "config/config.json"),
  };
  for (const source of Object.values(sources)) {
    if (!(await isFile(source))) {
      throw new Error(
        "Native input artifact is missing. Run `pnpm native:configure && pnpm native:build`, then retry.",
      );
    }
  }

  const staged = {
    runtime: path.join(repositoryRoot, plan.runtimeDestination),
    plugin: path.join(repositoryRoot, plan.pluginDestination),
    config: path.join(repositoryRoot, plan.configDestination),
  };
  for (const [kind, destination] of Object.entries(staged)) {
    await mkdir(path.dirname(destination), { recursive: true });
    await copyFile(sources[kind], destination);
  }
  return staged;
}

async function isFile(candidate) {
  try {
    return (await stat(candidate)).isFile();
  } catch {
    return false;
  }
}

function hostTarget() {
  const version = execFileSync("rustc", ["-vV"], { encoding: "utf8" });
  const host = version
    .split(/\r?\n/u)
    .find((line) => line.startsWith("host: "))
    ?.slice("host: ".length);
  if (!host || !VALID_TARGET.test(host)) {
    throw new Error("rustc did not report a valid host target triple");
  }
  return host;
}

function parseArguments(argv) {
  let profile = "Debug";
  let target;
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--profile") {
      profile = argv[index + 1];
      index += 1;
    } else if (argument === "--target") {
      target = argv[index + 1];
      index += 1;
    } else {
      throw new Error(`unknown argument: ${argument}`);
    }
  }
  return { profile, target: target ?? hostTarget() };
}

const scriptPath = fileURLToPath(import.meta.url);
if (process.argv[1] && path.resolve(process.argv[1]) === scriptPath) {
  const repositoryRoot = path.resolve(path.dirname(scriptPath), "../..");
  try {
    const options = parseArguments(process.argv.slice(2));
    const staged = await stageNativeArtifacts({ repositoryRoot, ...options });
    process.stdout.write(
      `Staged desktop native artifacts for ${options.target} (${options.profile}):\n` +
        `${staged.runtime}\n${staged.plugin}\n${staged.config}\n`,
    );
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  }
}
