import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

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

export function validatePhase05Acceptance({ repositoryRoot, markdown, planMarkdown }) {
  if (!path.isAbsolute(repositoryRoot)) throw new Error("repository root must be absolute");
  if (/Phase\s+0\.5\s+complete/iu.test(markdown)) {
    throw new Error("acceptance report must not claim the blocked phase is complete");
  }
  assertUncheckedPlanTask(planMarkdown, "在 `.github/workflows/build.yml`", 18);
  assertUncheckedPlanTask(planMarkdown, "执行 Phase 0.5 最终验收", 20);

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
  for (const row of artifacts) {
    if (row.length !== 6 || row[2] !== path.posix.join(evidenceRoot, row[1])) {
      throw new Error(`artifact absolute path does not match repository path: ${row[1]}`);
    }
    if (!Number.isSafeInteger(Number(row[3])) || Number(row[3]) <= 0) {
      throw new Error(`artifact byte count is invalid: ${row[1]}`);
    }
    if (!/^[0-9a-f]{64}$/u.test(row[4]) || !row[5]) {
      throw new Error(`artifact digest or file type is invalid: ${row[1]}`);
    }
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

const scriptPath = fileURLToPath(import.meta.url);
if (process.argv[1] && path.resolve(process.argv[1]) === scriptPath) {
  const repositoryRoot = path.resolve(process.argv[2] ?? path.join(path.dirname(scriptPath), "../.."));
  try {
    const result = validatePhase05Acceptance({
      repositoryRoot,
      markdown: await readFile(
        path.join(repositoryRoot, "docs/development/PHASE-0.5-ACCEPTANCE.md"),
        "utf8",
      ),
      planMarkdown: await readFile(
        path.join(repositoryRoot, "plans/2026-08-19-phase-0-5-infrastructure-v1.md"),
        "utf8",
      ),
    });
    process.stdout.write(`${JSON.stringify(result)}\n`);
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  }
}
