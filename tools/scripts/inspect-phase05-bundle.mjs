import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { lstat, readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { parse as parseYaml } from "yaml";

import { runDoctor } from "./doctor.mjs";
import { verifyDesktopBundle } from "./verify-desktop-bundle.mjs";

const MACOS_TARGET = "aarch64-apple-darwin";
const forbiddenPackagedExtension =
  /\.(?:a|ckpt|dll|flac|gguf|h5|mp3|onnx|ort|pt|pth|py|pyc|safetensors|so|tflite|wav)$/iu;
const forbiddenDependency =
  /(?:python|pytorch|libtorch|cuda|cudnn|cublas|onnx|coreaudio|audiounit|audiotoolbox|portaudio|openal|cubeb)/iu;
const forbiddenSymbol =
  /(?:^|_)Py(?:_|[A-Z])|(?:^|_)OrtGetApiBase|(?:^|_)(?:cuda|cudnn|cublas)|(?:^|_)cu(?:Init|Device|Ctx|Mem|Module|Launch|Stream)|torch|c10::|(?:^|_)Audio(?:Object|Unit|Queue|Device)(?:Get|Set|Create|Destroy|Start|Stop|Initialize|Uninitialize|Add|Remove|Translate)|(?:^|_)Pa_OpenStream|(?:^|_)alcOpenDevice|(?:^|_)AVAudioEngine/iu;
const mintedToolObservations = new WeakSet();

export function expectedPhase05BundleFiles(target) {
  if (target !== MACOS_TARGET) throw new Error(`unsupported acceptance target: ${target}`);
  return [
    "Contents/Info.plist",
    "Contents/MacOS/ai-voice-studio",
    "Contents/MacOS/voice-runtime",
    "Contents/Resources/AI Voice Studio.icns",
    "Contents/Resources/config/config.json",
    `Contents/Resources/native/aivs_mock_voice_engine-${target}.dylib`,
  ];
}

export async function readPhase05DeclaredVersions(repositoryRoot) {
  const [nodeVersion, rootPackageText, rustToolchain, workflowText, projectVersion, versionsText, desktopPackageText, tauriConfigText] =
    await Promise.all([
      readFile(path.join(repositoryRoot, ".node-version"), "utf8"),
      readFile(path.join(repositoryRoot, "package.json"), "utf8"),
      readFile(path.join(repositoryRoot, "rust-toolchain.toml"), "utf8"),
      readFile(path.join(repositoryRoot, ".github/workflows/build.yml"), "utf8"),
      readFile(path.join(repositoryRoot, "VERSION"), "utf8"),
      readFile(path.join(repositoryRoot, "core/contracts/version.json"), "utf8"),
      readFile(path.join(repositoryRoot, "apps/desktop/package.json"), "utf8"),
      readFile(path.join(repositoryRoot, "apps/desktop/src-tauri/tauri.conf.json"), "utf8"),
    ]);
  const rootPackage = JSON.parse(rootPackageText);
  const versions = JSON.parse(versionsText);
  const desktopPackage = JSON.parse(desktopPackageText);
  const tauriConfig = JSON.parse(tauriConfigText);
  const workflow = parseYaml(workflowText);
  const nativeSetup = workflow.jobs["macos-arm64"].steps.find((step) =>
    step.uses?.startsWith("lukka/get-cmake@"),
  );
  const product = projectVersion.trim();
  const pnpm = rootPackage.packageManager?.match(/^pnpm@(\d+\.\d+\.\d+)$/u)?.[1];
  const rust = rustToolchain.match(/^channel\s*=\s*"([^"]+)"/mu)?.[1];
  if (
    !product ||
    desktopPackage.version !== product ||
    tauriConfig.version !== product ||
    !pnpm ||
    !rust ||
    typeof nativeSetup?.with?.cmakeVersion !== "string" ||
    typeof nativeSetup?.with?.ninjaVersion !== "string"
  ) {
    throw new Error("Phase 0.5 version authorities are incomplete or inconsistent");
  }
  return {
    product: {
      declared: product,
      authority: "VERSION + apps/desktop/package.json + tauri.conf.json",
    },
    runtime: {
      declared: versions.runtime.version,
      authority: "core/contracts/version.json",
    },
    ipcProtocol: {
      declared: String(versions.ipc_protocol.current_version),
      authority: "core/contracts/version.json",
    },
    voiceEngineAbi: {
      declared: String(versions.voice_engine_abi.current_version),
      authority: "core/contracts/version.json",
    },
    tools: {
      node: { declared: nodeVersion.trim(), authority: ".node-version" },
      pnpm: { declared: pnpm, authority: "package.json#packageManager" },
      rust: { declared: rust, authority: "rust-toolchain.toml" },
      cmake: {
        declared: nativeSetup.with.cmakeVersion,
        authority: ".github/workflows/build.yml",
      },
      ninja: {
        declared: nativeSetup.with.ninjaVersion,
        authority: ".github/workflows/build.yml",
      },
    },
  };
}

export async function collectPhase05VersionEvidence(
  repositoryRoot,
  bundleRoot,
  { doctorRunner = runDoctor, toolRunner = runTool } = {},
) {
  const declared = await readPhase05DeclaredVersions(repositoryRoot);
  const doctor = doctorRunner({ repositoryRoot });
  if (doctor.exitCode !== 0) throw new Error("doctor must pass before acceptance evidence is generated");
  const checks = new Map(doctor.checks.map((check) => [check.id, check]));
  const observedTool = (id, expected) => {
    const check = checks.get(id);
    if (check?.status !== "pass" || check.detail !== expected) {
      throw new Error(`observed version drift for ${id}: expected ${expected}`);
    }
    return check.detail;
  };
  const productObserved = toolRunner("/usr/libexec/PlistBuddy", [
    "-c",
    "Print :CFBundleShortVersionString",
    path.join(bundleRoot, "Contents/Info.plist"),
  ]).trim();
  if (productObserved !== declared.product.declared) {
    throw new Error(
      `observed product version drift: expected ${declared.product.declared}`,
    );
  }
  const rustcObserved = observedTool("rustc", declared.tools.rust.declared);
  const cargoObserved = observedTool("cargo", declared.tools.rust.declared);
  if (rustcObserved !== cargoObserved) throw new Error("rustc and Cargo version evidence diverged");
  return {
    product: {
      declared: declared.product.declared,
      observed: productObserved,
      authority: `${declared.product.authority} + bundle Info.plist`,
    },
    runtime: { ...declared.runtime, observed: declared.runtime.declared },
    ipcProtocol: { ...declared.ipcProtocol, observed: declared.ipcProtocol.declared },
    voiceEngineAbi: {
      ...declared.voiceEngineAbi,
      observed: declared.voiceEngineAbi.declared,
    },
    tools: {
      node: {
        ...declared.tools.node,
        observed: observedTool("node", declared.tools.node.declared),
      },
      pnpm: {
        ...declared.tools.pnpm,
        observed: observedTool("pnpm", declared.tools.pnpm.declared),
      },
      rust: { ...declared.tools.rust, observed: rustcObserved },
      cmake: {
        ...declared.tools.cmake,
        observed: observedTool("cmake", declared.tools.cmake.declared),
      },
      ninja: {
        ...declared.tools.ninja,
        observed: observedTool("ninja", declared.tools.ninja.declared),
      },
    },
  };
}

function expectedMachOBinaries(target) {
  return [
    "Contents/MacOS/ai-voice-studio",
    "Contents/MacOS/voice-runtime",
    `Contents/Resources/native/aivs_mock_voice_engine-${target}.dylib`,
  ];
}

function isAllowedSystemDependency(dependency, binary) {
  if (dependency.startsWith("/System/Library/") || dependency.startsWith("/usr/lib/")) {
    return true;
  }
  return (
    binary.endsWith(".dylib") && dependency === "@rpath/libaivs_mock_voice_engine.dylib"
  );
}

export function validatePhase05BundleEvidence(evidence) {
  if (evidence?.schemaVersion !== 1 || evidence.target !== MACOS_TARGET) {
    throw new Error("unsupported Phase 0.5 bundle evidence schema or target");
  }
  if (!path.isAbsolute(evidence.bundleRoot)) throw new Error("bundle root must be absolute");

  const expectedFiles = expectedPhase05BundleFiles(evidence.target);
  const actualFiles = evidence.inventory.map((entry) => entry.relativePath).sort();
  const unexpected = actualFiles.find((entry) => !expectedFiles.includes(entry));
  if (unexpected) throw new Error(`unexpected bundle file: ${unexpected}`);
  if (JSON.stringify(actualFiles) !== JSON.stringify([...expectedFiles].sort())) {
    throw new Error("bundle inventory mismatch: a required Phase 0.5 artifact is missing");
  }
  for (const entry of evidence.inventory) {
    if (entry.symlink) throw new Error(`bundle symlink is forbidden: ${entry.relativePath}`);
    if (!Number.isSafeInteger(entry.bytes) || entry.bytes <= 0) {
      throw new Error(`bundle file size is invalid: ${entry.relativePath}`);
    }
    if (!/^[0-9a-f]{64}$/u.test(entry.sha256)) {
      throw new Error(`bundle digest is invalid: ${entry.relativePath}`);
    }
    if (forbiddenPackagedExtension.test(entry.relativePath)) {
      throw new Error(`forbidden model, runtime, archive, or audio asset: ${entry.relativePath}`);
    }
  }

  const expectedBinaries = expectedMachOBinaries(evidence.target);
  const actualBinaries = evidence.binaries.map((binary) => binary.relativePath).sort();
  if (JSON.stringify(actualBinaries) !== JSON.stringify([...expectedBinaries].sort())) {
    throw new Error("Mach-O evidence does not cover the exact executable/plugin set");
  }
  for (const binary of evidence.binaries) {
    assertCompletedMachOInspection(evidence.bundleRoot, binary);
    const inventory = evidence.inventory.find((entry) => entry.relativePath === binary.relativePath);
    if (!inventory?.fileType.includes("Mach-O")) {
      throw new Error(`expected Mach-O file classification: ${binary.relativePath}`);
    }
    if (JSON.stringify(binary.architectures) !== JSON.stringify(["arm64"])) {
      throw new Error(`Mach-O is not exactly arm64: ${binary.relativePath}`);
    }
    for (const dependency of binary.dependencies) {
      if (forbiddenDependency.test(dependency)) {
        throw new Error(`forbidden dynamic dependency in ${binary.relativePath}: ${dependency}`);
      }
      if (!isAllowedSystemDependency(dependency, binary.relativePath)) {
        throw new Error(`non-system dynamic dependency in ${binary.relativePath}: ${dependency}`);
      }
    }
    for (const symbol of [...binary.undefinedSymbols, ...binary.globalSymbols]) {
      if (forbiddenSymbol.test(symbol)) {
        throw new Error(`forbidden runtime symbol in ${binary.relativePath}: ${symbol}`);
      }
    }
  }

  return {
    ...evidence,
    exclusions: {
      bundledPythonOrAiRuntime: false,
      bundledModelOrAudioAsset: false,
      forbiddenDynamicDependency: false,
      forbiddenRuntimeSymbol: false,
      nonArm64MachO: false,
    },
  };
}

function countSymbols(binaries, field) {
  const symbols = binaries.flatMap((binary) => binary[field]);
  return { observations: symbols.length, unique: new Set(symbols).size };
}

function manifestInspection(observation) {
  return {
    tool: observation.tool,
    arguments: observation.arguments.slice(0, -1),
    evidencePresent: true,
    completed: true,
  };
}

export function buildPhase05AcceptanceManifest(evidence, versions) {
  const validated = validatePhase05BundleEvidence(evidence);
  const undefinedSymbols = countSymbols(validated.binaries, "undefinedSymbols");
  const globalSymbols = countSymbols(validated.binaries, "globalSymbols");
  return {
    schemaVersion: 1,
    target: validated.target,
    bundlePath: "target/release/bundle/macos/AI Voice Studio.app",
    artifacts: validated.inventory.map(({ relativePath, bytes, sha256, fileType }) => ({
      relativePath,
      bytes,
      sha256,
      fileType,
    })),
    binaries: validated.binaries.map((binary) => ({
      relativePath: binary.relativePath,
      architectures: binary.architectures,
      dependencies: binary.dependencies,
      symbolCounts: {
        undefinedObservations: binary.undefinedSymbols.length,
        undefinedUnique: new Set(binary.undefinedSymbols).size,
        globalObservations: binary.globalSymbols.length,
        globalUnique: new Set(binary.globalSymbols).size,
      },
      inspections: {
        architectures: manifestInspection(binary.inspections.architectures),
        dependencies: manifestInspection(binary.inspections.dependencies),
        undefinedSymbols: manifestInspection(binary.inspections.undefinedSymbols),
        globalSymbols: manifestInspection(binary.inspections.globalSymbols),
      },
    })),
    symbolTotals: {
      undefinedObservations: undefinedSymbols.observations,
      undefinedUnique: undefinedSymbols.unique,
      globalObservations: globalSymbols.observations,
      globalUnique: globalSymbols.unique,
    },
    versions,
    exclusions: validated.exclusions,
  };
}

function runTool(executable, toolArguments) {
  return execFileSync(executable, toolArguments, {
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
    stdio: ["ignore", "pipe", "pipe"],
  });
}

function observeTool(executable, toolArguments, toolRunner) {
  let observation;
  try {
    observation = {
      tool: executable,
      arguments: [...toolArguments],
      evidencePresent: true,
      completed: true,
      output: toolRunner(executable, toolArguments),
    };
  } catch (error) {
    observation = {
      tool: executable,
      arguments: [...toolArguments],
      evidencePresent: false,
      completed: false,
      error: error instanceof Error ? error.message : String(error),
    };
  }
  mintedToolObservations.add(observation);
  return observation;
}

async function bundleInventory(bundleRoot) {
  const inventory = [];
  async function visit(directory) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const absolutePath = path.join(directory, entry.name);
      const relativePath = path.relative(bundleRoot, absolutePath).split(path.sep).join("/");
      const metadata = await lstat(absolutePath);
      if (metadata.isDirectory()) {
        await visit(absolutePath);
      } else {
        const bytes = await readFile(absolutePath);
        inventory.push({
          relativePath,
          bytes: metadata.size,
          sha256: createHash("sha256").update(bytes).digest("hex"),
          fileType: runTool("/usr/bin/file", ["-b", absolutePath]).trim(),
          ...(metadata.isSymbolicLink() ? { symlink: true } : {}),
        });
      }
    }
  }
  await visit(bundleRoot);
  return inventory.sort((left, right) => left.relativePath.localeCompare(right.relativePath));
}

function parseDependencies(output) {
  return output
    .split(/\r?\n/u)
    .slice(1)
    .map((line) => line.trim().split(" (compatibility version", 1)[0])
    .filter(Boolean);
}

function parseSymbols(output) {
  return output
    .split(/\r?\n/u)
    .map((line) => line.trim().split(/\s+/u).at(-1))
    .filter(Boolean);
}

function decodedObservation(observation, parser) {
  return observation.completed ? parser(observation.output) : [];
}

export function collectMachOEvidence(
  bundleRoot,
  relativePath,
  toolRunner = runTool,
) {
  const absolutePath = path.join(bundleRoot, relativePath);
  const inspections = {
    architectures: observeTool("/usr/bin/lipo", ["-archs", absolutePath], toolRunner),
    dependencies: observeTool("/usr/bin/otool", ["-L", absolutePath], toolRunner),
    undefinedSymbols: observeTool("/usr/bin/nm", ["-u", absolutePath], toolRunner),
    globalSymbols: observeTool("/usr/bin/nm", ["-g", absolutePath], toolRunner),
  };
  return {
    relativePath,
    inspections,
    architectures: decodedObservation(inspections.architectures, (output) =>
      output.trim().split(/\s+/u).filter(Boolean),
    ),
    dependencies: decodedObservation(inspections.dependencies, parseDependencies),
    undefinedSymbols: decodedObservation(inspections.undefinedSymbols, parseSymbols),
    globalSymbols: decodedObservation(inspections.globalSymbols, parseSymbols),
  };
}

function assertCompletedObservation(observation, tool, toolArguments, decoded, parser) {
  if (
    !observation ||
    !mintedToolObservations.has(observation) ||
    observation.evidencePresent !== true ||
    observation.completed !== true ||
    observation.tool !== tool ||
    JSON.stringify(observation.arguments) !== JSON.stringify(toolArguments) ||
    typeof observation.output !== "string" ||
    JSON.stringify(decoded) !== JSON.stringify(parser(observation.output))
  ) {
    throw new Error("completed Mach-O inspection evidence is required");
  }
}

function assertCompletedMachOInspection(bundleRoot, binary) {
  const absolutePath = path.join(bundleRoot, binary.relativePath);
  assertCompletedObservation(
    binary.inspections?.architectures,
    "/usr/bin/lipo",
    ["-archs", absolutePath],
    binary.architectures,
    (output) => output.trim().split(/\s+/u).filter(Boolean),
  );
  assertCompletedObservation(
    binary.inspections?.dependencies,
    "/usr/bin/otool",
    ["-L", absolutePath],
    binary.dependencies,
    parseDependencies,
  );
  assertCompletedObservation(
    binary.inspections?.undefinedSymbols,
    "/usr/bin/nm",
    ["-u", absolutePath],
    binary.undefinedSymbols,
    parseSymbols,
  );
  assertCompletedObservation(
    binary.inspections?.globalSymbols,
    "/usr/bin/nm",
    ["-g", absolutePath],
    binary.globalSymbols,
    parseSymbols,
  );
}

export async function inspectPhase05Bundle(bundleRoot, target = MACOS_TARGET) {
  const absoluteRoot = path.resolve(bundleRoot);
  await verifyDesktopBundle(absoluteRoot, target);
  const evidence = {
    schemaVersion: 1,
    target,
    bundleRoot: absoluteRoot,
    inventory: await bundleInventory(absoluteRoot),
    binaries: expectedMachOBinaries(target).map((relativePath) =>
      collectMachOEvidence(absoluteRoot, relativePath),
    ),
  };
  return validatePhase05BundleEvidence(evidence);
}

export async function createPhase05AcceptanceManifest(
  bundleRoot,
  target = MACOS_TARGET,
  repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../.."),
) {
  const evidence = await inspectPhase05Bundle(bundleRoot, target);
  const versions = await collectPhase05VersionEvidence(
    repositoryRoot,
    path.resolve(bundleRoot),
  );
  return buildPhase05AcceptanceManifest(evidence, versions);
}

const scriptPath = fileURLToPath(import.meta.url);
if (process.argv[1] && path.resolve(process.argv[1]) === scriptPath) {
  const [bundleRoot, target = MACOS_TARGET, ...options] = process.argv.slice(2);
  const manifestFlag = options.indexOf("--write-manifest");
  const manifestPath = manifestFlag >= 0 ? options[manifestFlag + 1] : undefined;
  if (!bundleRoot) {
    process.stderr.write(
      "usage: inspect-phase05-bundle.mjs <macos-app-path> [target] [--write-manifest <path>]\n",
    );
    process.exitCode = 2;
  } else if (manifestFlag >= 0 && !manifestPath) {
    process.stderr.write("--write-manifest requires an output path\n");
    process.exitCode = 2;
  } else {
    try {
      if (manifestPath) {
        const manifest = await createPhase05AcceptanceManifest(bundleRoot, target);
        const absoluteManifestPath = path.resolve(manifestPath);
        await writeFile(absoluteManifestPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
        process.stdout.write(`Wrote deterministic acceptance manifest: ${absoluteManifestPath}\n`);
      } else {
        const evidence = await inspectPhase05Bundle(bundleRoot, target);
        process.stdout.write(`${JSON.stringify(evidence, null, 2)}\n`);
      }
    } catch (error) {
      process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
      process.exitCode = 1;
    }
  }
}
