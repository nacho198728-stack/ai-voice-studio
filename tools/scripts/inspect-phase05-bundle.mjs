import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { lstat, readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { verifyDesktopBundle } from "./verify-desktop-bundle.mjs";

const MACOS_TARGET = "aarch64-apple-darwin";
const forbiddenPackagedExtension =
  /\.(?:a|ckpt|dll|flac|gguf|h5|mp3|onnx|ort|pt|pth|py|pyc|safetensors|so|tflite|wav)$/iu;
const forbiddenDependency =
  /(?:python|pytorch|libtorch|cuda|cudnn|cublas|onnx|coreaudio|audiounit|audiotoolbox|portaudio|openal|cubeb)/iu;
const forbiddenSymbol =
  /(?:^|_)Py(?:_|[A-Z])|(?:^|_)OrtGetApiBase|(?:^|_)(?:cuda|cudnn|cublas)|(?:^|_)cu(?:Init|Device|Ctx|Mem|Module|Launch|Stream)|torch|c10::|(?:^|_)Audio(?:Object|Unit|Queue|Device)(?:Get|Set|Create|Destroy|Start|Stop|Initialize|Uninitialize|Add|Remove|Translate)|(?:^|_)Pa_OpenStream|(?:^|_)alcOpenDevice|(?:^|_)AVAudioEngine/iu;

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
    for (const symbol of [...(binary.undefinedSymbols ?? []), ...(binary.globalSymbols ?? [])]) {
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

function runTool(executable, toolArguments) {
  return execFileSync(executable, toolArguments, {
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
    stdio: ["ignore", "pipe", "pipe"],
  });
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

function inspectMachO(bundleRoot, relativePath) {
  const absolutePath = path.join(bundleRoot, relativePath);
  return {
    relativePath,
    architectures: runTool("/usr/bin/lipo", ["-archs", absolutePath]).trim().split(/\s+/u),
    dependencies: parseDependencies(runTool("/usr/bin/otool", ["-L", absolutePath])),
    undefinedSymbols: parseSymbols(runTool("/usr/bin/nm", ["-u", absolutePath])),
    globalSymbols: parseSymbols(runTool("/usr/bin/nm", ["-g", absolutePath])),
  };
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
      inspectMachO(absoluteRoot, relativePath),
    ),
  };
  return validatePhase05BundleEvidence(evidence);
}

const scriptPath = fileURLToPath(import.meta.url);
if (process.argv[1] && path.resolve(process.argv[1]) === scriptPath) {
  const [bundleRoot, target = MACOS_TARGET] = process.argv.slice(2);
  if (!bundleRoot) {
    process.stderr.write("usage: inspect-phase05-bundle.mjs <macos-app-path> [target]\n");
    process.exitCode = 2;
  } else {
    try {
      const evidence = await inspectPhase05Bundle(bundleRoot, target);
      process.stdout.write(`${JSON.stringify(evidence, null, 2)}\n`);
    } catch (error) {
      process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
      process.exitCode = 1;
    }
  }
}
