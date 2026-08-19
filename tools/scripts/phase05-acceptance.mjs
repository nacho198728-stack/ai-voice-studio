import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";

import { readPhase05DeclaredVersions } from "./inspect-phase05-bundle.mjs";

const requirementIds = new Set([
  "MONO",
  "CROSS",
  "UI",
  "PROC",
  "IPC",
  "MOCK",
  "ABI",
  "PCM",
  "LOG",
  "CONFIG",
  "TEST",
  "ADR",
  "EXCL",
]);
const artifactPaths = [
  "target/release/bundle/macos/AI Voice Studio.app/Contents/Info.plist",
  "target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/ai-voice-studio",
  "target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/voice-runtime",
  "target/release/bundle/macos/AI Voice Studio.app/Contents/Resources/AI Voice Studio.icns",
  "target/release/bundle/macos/AI Voice Studio.app/Contents/Resources/config/config.json",
  "target/release/bundle/macos/AI Voice Studio.app/Contents/Resources/native/aivs_mock_voice_engine-aarch64-apple-darwin.dylib",
];
const binaryPaths = [
  "Contents/MacOS/ai-voice-studio",
  "Contents/MacOS/voice-runtime",
  "Contents/Resources/native/aivs_mock_voice_engine-aarch64-apple-darwin.dylib",
];
const inspectionKeys = [
  "architectures",
  "dependencies",
  "undefinedSymbols",
  "globalSymbols",
];
const symbolCountKeys = [
  "globalTotal",
  "globalUnique",
  "undefinedTotal",
  "undefinedUnique",
];
const expectedInspections = {
  architectures: { tool: "/usr/bin/lipo", arguments: ["-archs"] },
  dependencies: { tool: "/usr/bin/otool", arguments: ["-L"] },
  undefinedSymbols: { tool: "/usr/bin/nm", arguments: ["-u"] },
  globalSymbols: { tool: "/usr/bin/nm", arguments: ["-g"] },
};

function stripCode(value) {
  return value.replaceAll("`", "").trim();
}

function tableAfterHeading(markdown, heading) {
  const lines = markdown.split(/\r?\n/u);
  const headingIndex = lines.findIndex((line) => line.trim() === heading);
  if (headingIndex < 0) return [];
  let index = headingIndex + 1;
  while (index < lines.length && !lines[index].trim().startsWith("|")) index += 1;
  if (!lines[index + 1]?.match(/^\s*\|[\s:|-]+\|\s*$/u)) return [];
  const rows = [];
  for (index += 2; index < lines.length && lines[index].trim().startsWith("|"); index += 1) {
    rows.push(lines[index].trim().slice(1, -1).split("|").map(stripCode));
  }
  return rows;
}

function assertUncheckedPlanTask(planMarkdown, marker, task) {
  const line = planMarkdown.split(/\r?\n/u).find((candidate) => candidate.includes(marker));
  if (!line?.startsWith("- [ ] ")) throw new Error(`Task ${task} must remain unchecked`);
}

function same(left, right) {
  return isDeepStrictEqual(left, right);
}

function assertSymbolCounts(counts, { label, aggregate = false }) {
  const keys = counts && typeof counts === "object" ? Object.keys(counts).sort() : [];
  if (!same(keys, symbolCountKeys)) {
    throw new Error(
      aggregate
        ? "acceptance manifest requires exact aggregate symbol count keys"
        : `acceptance manifest requires exact symbol count keys: ${label}`,
    );
  }
  for (const key of symbolCountKeys) {
    if (!Number.isSafeInteger(counts[key]) || counts[key] < 0) {
      throw new Error(
        `acceptance manifest symbol count must be a nonnegative safe integer: ${label}.${key}`,
      );
    }
  }
  if (
    counts.undefinedUnique > counts.undefinedTotal ||
    counts.globalUnique > counts.globalTotal
  ) {
    throw new Error(
      aggregate
        ? "acceptance manifest aggregate unique symbol count exceeds total"
        : `acceptance manifest unique symbol count exceeds total: ${label}`,
    );
  }
}

function assertManifestVersions(manifest, declaredVersions) {
  const expected = {
    product: {
      ...declaredVersions.product,
      observed: declaredVersions.product.declared,
      authority: `${declaredVersions.product.authority} + bundle Info.plist`,
    },
    runtime: { ...declaredVersions.runtime, observed: declaredVersions.runtime.declared },
    ipcProtocol: {
      ...declaredVersions.ipcProtocol,
      observed: declaredVersions.ipcProtocol.declared,
    },
    voiceEngineAbi: {
      ...declaredVersions.voiceEngineAbi,
      observed: declaredVersions.voiceEngineAbi.declared,
    },
    tools: Object.fromEntries(
      Object.entries(declaredVersions.tools).map(([id, version]) => [
        id,
        { ...version, observed: version.declared },
      ]),
    ),
  };
  if (!same(manifest.versions, expected)) {
    throw new Error("acceptance manifest version authority drift");
  }
}

function assertManifestShape(manifest, declaredVersions) {
  if (
    manifest?.schemaVersion !== 1 ||
    manifest.target !== "aarch64-apple-darwin" ||
    manifest.bundlePath !== "target/release/bundle/macos/AI Voice Studio.app" ||
    "bundleRoot" in manifest ||
    "generatedAt" in manifest
  ) {
    throw new Error("acceptance manifest schema or deterministic path contract is invalid");
  }
  const manifestArtifacts = manifest.artifacts ?? [];
  const expectedRelativeArtifacts = artifactPaths.map((relativePath) =>
    relativePath.slice(`${manifest.bundlePath}/`.length),
  );
  if (
    manifestArtifacts.length !== expectedRelativeArtifacts.length ||
    !same(
      manifestArtifacts.map((artifact) => artifact.relativePath).sort(),
      [...expectedRelativeArtifacts].sort(),
    )
  ) {
    throw new Error("acceptance manifest artifact inventory is invalid");
  }
  for (const artifact of manifestArtifacts) {
    if (
      !Number.isSafeInteger(artifact.bytes) ||
      artifact.bytes <= 0 ||
      !/^[0-9a-f]{64}$/u.test(artifact.sha256) ||
      typeof artifact.fileType !== "string" ||
      !artifact.fileType
    ) {
      throw new Error(`acceptance manifest artifact metadata is invalid: ${artifact.relativePath}`);
    }
  }

  const binaries = manifest.binaries ?? [];
  if (
    binaries.length !== binaryPaths.length ||
    !same(binaries.map((binary) => binary.relativePath).sort(), [...binaryPaths].sort())
  ) {
    throw new Error("acceptance manifest binary inventory is invalid");
  }
  for (const binary of binaries) {
    if (!same(binary.architectures, ["arm64"]) || !Array.isArray(binary.dependencies)) {
      throw new Error(`acceptance manifest binary evidence is invalid: ${binary.relativePath}`);
    }
    for (const key of inspectionKeys) {
      const inspection = binary.inspections?.[key];
      const expectedInspection = expectedInspections[key];
      if (
        inspection?.evidencePresent !== true ||
        inspection.completed !== true ||
        inspection.tool !== expectedInspection.tool ||
        !same(inspection.arguments, expectedInspection.arguments)
      ) {
        throw new Error(
          `acceptance manifest inspection provenance is incomplete: ${binary.relativePath}`,
        );
      }
    }
    assertSymbolCounts(binary.symbolCounts, { label: binary.relativePath });
  }
  assertSymbolCounts(manifest.symbolTotals, { label: "symbolTotals", aggregate: true });
  const summedUndefined = binaries.reduce(
    (total, binary) => total + binary.symbolCounts.undefinedTotal,
    0,
  );
  const summedGlobal = binaries.reduce(
    (total, binary) => total + binary.symbolCounts.globalTotal,
    0,
  );
  if (
    manifest.symbolTotals.undefinedTotal !== summedUndefined ||
    manifest.symbolTotals.globalTotal !== summedGlobal
  ) {
    throw new Error("acceptance manifest aggregate symbol totals do not equal the binary sums");
  }
  if (
    !same(manifest.exclusions, {
      bundledPythonOrAiRuntime: false,
      bundledModelOrAudioAsset: false,
      forbiddenDynamicDependency: false,
      forbiddenRuntimeSymbol: false,
      nonArm64MachO: false,
    })
  ) {
    throw new Error("acceptance manifest exclusion result is incomplete");
  }
  assertManifestVersions(manifest, declaredVersions);
}

function reportVersionRows(manifest) {
  const entries = [
    ["Product", manifest.versions.product],
    ["Runtime", manifest.versions.runtime],
    ["IPC protocol", manifest.versions.ipcProtocol],
    ["VoiceEngine ABI", manifest.versions.voiceEngineAbi],
    ["Node.js", manifest.versions.tools.node],
    ["pnpm", manifest.versions.tools.pnpm],
    ["Rust", manifest.versions.tools.rust],
    ["CMake", manifest.versions.tools.cmake],
    ["Ninja", manifest.versions.tools.ninja],
  ];
  return entries.map(([label, version]) => [
    label,
    version.declared,
    version.observed,
    version.authority,
  ]);
}

function reportInspectionProvenance(binary) {
  return inspectionKeys
    .map((key) => {
      const inspection = binary.inspections[key];
      return `${path.posix.basename(inspection.tool)} ${inspection.arguments.join(" ")}`.trim();
    })
    .join("; ") + " — completed";
}

function reportBinaryRows(manifest) {
  return manifest.binaries.map((binary) => [
    `${manifest.bundlePath}/${binary.relativePath}`,
    binary.architectures.join(","),
    String(binary.dependencies.length),
    String(binary.symbolCounts.undefinedTotal),
    String(binary.symbolCounts.undefinedUnique),
    String(binary.symbolCounts.globalTotal),
    String(binary.symbolCounts.globalUnique),
    reportInspectionProvenance(binary),
  ]);
}

function reportDependencyRows(manifest) {
  return manifest.binaries.flatMap((binary) =>
    binary.dependencies.map((dependency) => [
      `${manifest.bundlePath}/${binary.relativePath}`,
      dependency,
    ]),
  );
}

export function validatePhase05Acceptance({
  repositoryRoot,
  markdown,
  planMarkdown,
  manifest,
  declaredVersions,
}) {
  if (!path.isAbsolute(repositoryRoot)) throw new Error("repository root must be absolute");
  if (/Phase\s+0\.5\s+complete/iu.test(markdown)) {
    throw new Error("acceptance report must not claim the blocked phase is complete");
  }
  assertUncheckedPlanTask(planMarkdown, "在 `.github/workflows/build.yml`", 18);
  assertUncheckedPlanTask(planMarkdown, "执行 Phase 0.5 最终验收", 20);
  assertManifestShape(manifest, declaredVersions);

  const statusRows = new Map(
    tableAfterHeading(markdown, "## Gate status").map((row) => [row[0], row[1]]),
  );
  if (statusRows.get("Overall") !== "PENDING") {
    throw new Error("overall acceptance status must be PENDING");
  }
  if (statusRows.get("Only blocker") !== "Real Windows x64 workflow run") {
    throw new Error("the sole blocker must be the real Windows x64 workflow run");
  }
  const evidenceRoot = statusRows.get("Local repository root");
  if (!evidenceRoot?.startsWith("/") || path.posix.normalize(evidenceRoot) !== evidenceRoot) {
    throw new Error("local macOS evidence root must be a normalized absolute path");
  }

  const matrix = tableAfterHeading(markdown, "## Verification matrix");
  const ids = matrix.map((row) => row[0]);
  if (
    ids.length !== requirementIds.size ||
    ids.some((id) => !requirementIds.has(id)) ||
    new Set(ids).size !== requirementIds.size
  ) {
    throw new Error("verification matrix must cover the exact 13 Phase 0.5 requirements");
  }
  for (const row of matrix) {
    if (row.length !== 5 || !row[1] || row[2] !== "PASS" || !row[3] || row[4] !== "PENDING") {
      throw new Error(`verification matrix row ${row[0]} has invalid evidence or status`);
    }
  }

  const windowsRows = tableAfterHeading(markdown, "## Windows CI evidence");
  const windows = windowsRows.find((row) => row[0] === "Windows x64");
  if (
    windows?.length !== 3 ||
    windows[1] !== "PENDING" ||
    windows[2] !== "PENDING — no run URL available"
  ) {
    throw new Error("Windows x64 evidence must remain pending without a fabricated URL");
  }

  const artifacts = tableAfterHeading(markdown, "## Release bundle artifacts");
  const actualRelativePaths = artifacts.map((row) => row[1]);
  if (
    actualRelativePaths.length !== artifactPaths.length ||
    artifactPaths.some((relativePath) => !actualRelativePaths.includes(relativePath))
  ) {
    throw new Error("release artifact table must cover the exact bundle inventory");
  }
  const artifactsByPath = new Map(
    manifest.artifacts.map((artifact) => [
      `${manifest.bundlePath}/${artifact.relativePath}`,
      artifact,
    ]),
  );
  for (const row of artifacts) {
    if (row.length !== 6 || row[2] !== path.posix.join(evidenceRoot, row[1])) {
      throw new Error(`artifact absolute path does not match repository path: ${row[1]}`);
    }
    const artifact = artifactsByPath.get(row[1]);
    if (
      !artifact ||
      row[3] !== String(artifact.bytes) ||
      row[4] !== artifact.sha256 ||
      row[5] !== artifact.fileType
    ) {
      throw new Error(`artifact manifest mismatch: ${row[1]}`);
    }
  }

  if (!same(tableAfterHeading(markdown, "## Version evidence"), reportVersionRows(manifest))) {
    throw new Error("version manifest mismatch");
  }
  if (
    !same(
      tableAfterHeading(markdown, "## Mach-O inspection evidence"),
      reportBinaryRows(manifest),
    )
  ) {
    throw new Error("Mach-O inspection report does not match the manifest");
  }
  if (
    !same(
      tableAfterHeading(markdown, "## Mach-O dynamic dependencies"),
      reportDependencyRows(manifest),
    )
  ) {
    throw new Error("dynamic dependency report does not match the manifest");
  }
  const totals = manifest.symbolTotals;
  if (
    !same(tableAfterHeading(markdown, "## Symbol observation totals"), [
      [
        "All three Mach-O files (cross-file union for unique)",
        String(totals.undefinedTotal),
        String(totals.undefinedUnique),
        String(totals.globalTotal),
        String(totals.globalUnique),
      ],
    ])
  ) {
    throw new Error("symbol total report does not match the manifest");
  }

  for (const heading of ["## Known limitations", "## Phase 1 inputs", "## Minimal external action"]) {
    if (!markdown.includes(heading)) throw new Error(`acceptance report is missing ${heading}`);
  }
  return {
    status: "PENDING",
    windowsEvidenceUrl: null,
    requirementCount: matrix.length,
    artifactCount: artifacts.length,
  };
}

export async function loadPhase05Acceptance(repositoryRoot) {
  const [markdown, planMarkdown, manifestText, declaredVersions] = await Promise.all([
    readFile(path.join(repositoryRoot, "docs/development/PHASE-0.5-ACCEPTANCE.md"), "utf8"),
    readFile(
      path.join(repositoryRoot, "plans/2026-08-19-phase-0-5-infrastructure-v1.md"),
      "utf8",
    ),
    readFile(
      path.join(repositoryRoot, "docs/development/PHASE-0.5-ACCEPTANCE-EVIDENCE.json"),
      "utf8",
    ),
    readPhase05DeclaredVersions(repositoryRoot),
  ]);
  return { repositoryRoot, markdown, planMarkdown, manifest: JSON.parse(manifestText), declaredVersions };
}

const scriptPath = fileURLToPath(import.meta.url);
if (process.argv[1] && path.resolve(process.argv[1]) === scriptPath) {
  const repositoryRoot = path.resolve(process.argv[2] ?? path.join(path.dirname(scriptPath), "../.."));
  try {
    const result = validatePhase05Acceptance(await loadPhase05Acceptance(repositoryRoot));
    process.stdout.write(`${JSON.stringify(result)}\n`);
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  }
}
