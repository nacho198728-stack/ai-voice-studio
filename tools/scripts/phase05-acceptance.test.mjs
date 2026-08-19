import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  loadPhase05Acceptance,
  validatePhase05Acceptance,
} from "./phase05-acceptance.mjs";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

async function repositoryAcceptance() {
  return validatePhase05Acceptance(await loadPhase05Acceptance(repositoryRoot));
}

const exactSymbolCountKeys = [
  "globalTotal",
  "globalUnique",
  "undefinedTotal",
  "undefinedUnique",
];

function synchronizeFirstBinaryCounts(markdown, counts) {
  const prefix =
    "| target/release/bundle/macos/AI Voice Studio.app/Contents/MacOS/ai-voice-studio | arm64 | 13 |";
  const original = markdown.split(/\r?\n/u).find((line) => line.startsWith(prefix));
  assert.ok(original, "fixture must contain the desktop symbol row");
  const replacement = `${prefix} ${String(counts.undefinedTotal)} | ${String(counts.undefinedUnique)} | ${String(counts.globalTotal)} | ${String(counts.globalUnique)} | lipo -archs; otool -L; nm -u; nm -g — completed |`;
  return markdown.replace(original, replacement);
}

function synchronizeAggregateCounts(markdown, counts) {
  const prefix = "| All three Mach-O files (cross-file union for unique) |";
  const original = markdown.split(/\r?\n/u).find((line) => line.startsWith(prefix));
  assert.ok(original, "fixture must contain the aggregate symbol row");
  return markdown.replace(
    original,
    `${prefix} ${String(counts.undefinedTotal)} | ${String(counts.undefinedUnique)} | ${String(counts.globalTotal)} | ${String(counts.globalUnique)} |`,
  );
}

test("generated acceptance evidence uses the exact public symbol count schema", async () => {
  const { manifest } = await loadPhase05Acceptance(repositoryRoot);
  for (const binary of manifest.binaries) {
    assert.deepEqual(Object.keys(binary.symbolCounts).sort(), exactSymbolCountKeys);
  }
  assert.deepEqual(Object.keys(manifest.symbolTotals).sort(), exactSymbolCountKeys);
});

test("acceptance gate rejects missing or extra symbol count fields even when report rows match", async () => {
  const input = await loadPhase05Acceptance(repositoryRoot);
  for (const key of exactSymbolCountKeys) {
    const binaryManifest = structuredClone(input.manifest);
    delete binaryManifest.binaries[0].symbolCounts[key];
    assert.throws(
      () =>
        validatePhase05Acceptance({
          ...input,
          manifest: binaryManifest,
          markdown: synchronizeFirstBinaryCounts(
            input.markdown,
            binaryManifest.binaries[0].symbolCounts,
          ),
        }),
      /exact symbol count keys/u,
    );

    const aggregateManifest = structuredClone(input.manifest);
    delete aggregateManifest.symbolTotals[key];
    assert.throws(
      () =>
        validatePhase05Acceptance({
          ...input,
          manifest: aggregateManifest,
          markdown: synchronizeAggregateCounts(input.markdown, aggregateManifest.symbolTotals),
        }),
      /exact aggregate symbol count keys/u,
    );
  }

  for (const location of ["binary", "aggregate"]) {
    const manifest = structuredClone(input.manifest);
    const counts =
      location === "binary" ? manifest.binaries[0].symbolCounts : manifest.symbolTotals;
    counts.unexpected = 0;
    assert.throws(
      () =>
        validatePhase05Acceptance({
          ...input,
          manifest,
          markdown:
            location === "binary"
              ? synchronizeFirstBinaryCounts(input.markdown, counts)
              : synchronizeAggregateCounts(input.markdown, counts),
        }),
      location === "binary"
        ? /exact symbol count keys/u
        : /exact aggregate symbol count keys/u,
    );
  }
});

test("acceptance gate rejects unsafe symbol counts before trusting synchronized report text", async () => {
  const input = await loadPhase05Acceptance(repositoryRoot);
  for (const invalid of [-1, Number.NaN, 1.5, Number.MAX_SAFE_INTEGER + 1]) {
    const binaryManifest = structuredClone(input.manifest);
    binaryManifest.binaries[0].symbolCounts.undefinedTotal = invalid;
    assert.throws(
      () =>
        validatePhase05Acceptance({
          ...input,
          manifest: binaryManifest,
          markdown: synchronizeFirstBinaryCounts(
            input.markdown,
            binaryManifest.binaries[0].symbolCounts,
          ),
        }),
      /nonnegative safe integer/u,
    );

    const aggregateManifest = structuredClone(input.manifest);
    aggregateManifest.symbolTotals.globalTotal = invalid;
    assert.throws(
      () =>
        validatePhase05Acceptance({
          ...input,
          manifest: aggregateManifest,
          markdown: synchronizeAggregateCounts(input.markdown, aggregateManifest.symbolTotals),
        }),
      /nonnegative safe integer/u,
    );
  }
});

test("acceptance gate rejects impossible unique counts and aggregate total drift", async () => {
  const input = await loadPhase05Acceptance(repositoryRoot);

  const impossibleBinary = structuredClone(input.manifest);
  impossibleBinary.binaries[0].symbolCounts.undefinedUnique =
    impossibleBinary.binaries[0].symbolCounts.undefinedTotal + 1;
  assert.throws(
    () =>
      validatePhase05Acceptance({
        ...input,
        manifest: impossibleBinary,
        markdown: synchronizeFirstBinaryCounts(
          input.markdown,
          impossibleBinary.binaries[0].symbolCounts,
        ),
      }),
    /unique symbol count exceeds total/u,
  );

  const impossibleAggregate = structuredClone(input.manifest);
  impossibleAggregate.symbolTotals.globalUnique = impossibleAggregate.symbolTotals.globalTotal + 1;
  assert.throws(
    () =>
      validatePhase05Acceptance({
        ...input,
        manifest: impossibleAggregate,
        markdown: synchronizeAggregateCounts(input.markdown, impossibleAggregate.symbolTotals),
      }),
    /aggregate unique symbol count exceeds total/u,
  );

  const driftedAggregate = structuredClone(input.manifest);
  driftedAggregate.symbolTotals.undefinedTotal += 1;
  assert.throws(
    () =>
      validatePhase05Acceptance({
        ...input,
        manifest: driftedAggregate,
        markdown: synchronizeAggregateCounts(input.markdown, driftedAggregate.symbolTotals),
      }),
    /aggregate symbol totals do not equal the binary sums/u,
  );
});

test("checked-in acceptance remains pending only on an unrun real Windows job", async () => {
  const result = await repositoryAcceptance();
  assert.equal(result.status, "PENDING");
  assert.equal(result.windowsEvidenceUrl, null);
  assert.equal(result.requirementCount, 13);
  assert.equal(result.artifactCount, 6);
});

test("macOS absolute evidence remains auditable from a different checkout path", async () => {
  const input = await loadPhase05Acceptance(repositoryRoot);
  const result = validatePhase05Acceptance({
    ...input,
    repositoryRoot: "/github/workspace/other-checkout",
  });
  assert.equal(result.artifactCount, 6);
});

test("acceptance gate rejects a false Windows result or prematurely checked final plan tasks", async () => {
  const input = await loadPhase05Acceptance(repositoryRoot);

  assert.throws(
    () =>
      validatePhase05Acceptance({
        ...input,
        markdown: input.markdown.replace(
          "| Windows x64 | PENDING | PENDING — no run URL available |",
          "| Windows x64 | PASS | https://example.invalid/fake-run |",
        ),
      }),
    /Windows x64 evidence must remain pending/u,
  );
  assert.throws(
    () =>
      validatePhase05Acceptance({
        ...input,
        planMarkdown: input.planMarkdown.replace(
          "- [ ] 在 `.github/workflows/build.yml`",
          "- [x] 在 `.github/workflows/build.yml`",
        ),
      }),
    /Task 18 must remain unchecked/u,
  );
});

test("acceptance gate rejects an incomplete matrix and artifact path drift", async () => {
  const input = await loadPhase05Acceptance(repositoryRoot);

  assert.throws(
    () =>
      validatePhase05Acceptance({
        ...input,
        markdown: input.markdown.replace(/^\| EXCL \|.*\n/mu, ""),
      }),
    /verification matrix/u,
  );
  assert.throws(
    () =>
      validatePhase05Acceptance({
        ...input,
        markdown: input.markdown.replace(
          "/Users/alex/Documents/ChatGPT/AI翻唱/target/release/bundle/macos/AI Voice Studio.app/Contents/Info.plist",
          "/tmp/other.app/Contents/Info.plist",
        ),
      }),
    /artifact absolute path/u,
  );
});

test("acceptance gate rejects artifact and authoritative version drift from its manifest", async () => {
  const input = await loadPhase05Acceptance(repositoryRoot);
  const withMarkdown = (markdown) => ({ ...input, markdown });

  assert.throws(
    () =>
      validatePhase05Acceptance(
        withMarkdown(input.markdown.replace("cb73d729da55f14b514374da0c6c7f4f48b9af03651fe5a51c68420af85c3967", "f".repeat(64))),
      ),
    /artifact manifest mismatch/u,
  );
  assert.throws(
    () => validatePhase05Acceptance(withMarkdown(input.markdown.replace("| 1000 | cb73", "| 1001 | cb73"))),
    /artifact manifest mismatch/u,
  );
  assert.throws(
    () =>
      validatePhase05Acceptance(
        withMarkdown(input.markdown.replace("XML 1.0 document text, ASCII text", "forged file type")),
      ),
    /artifact manifest mismatch/u,
  );
  assert.throws(
    () =>
      validatePhase05Acceptance(
        withMarkdown(
          input.markdown.replace(
            "| Node.js | 24.16.0 | 24.16.0 |",
            "| Node.js | 0.0.0 | 0.0.0 |",
          ),
        ),
      ),
    /version manifest mismatch/u,
  );

  for (const [before, after] of [
    ["| Product | 0.0.0 | 0.0.0 |", "| Product | 9.9.9 | 9.9.9 |"],
    ["| Runtime | 0.0.0 | 0.0.0 |", "| Runtime | 9.9.9 | 9.9.9 |"],
    ["| IPC protocol | 1 | 1 |", "| IPC protocol | 9 | 9 |"],
    ["| VoiceEngine ABI | 1 | 1 |", "| VoiceEngine ABI | 9 | 9 |"],
    ["| pnpm | 11.19.0 | 11.19.0 |", "| pnpm | 9.9.9 | 9.9.9 |"],
    ["| Rust | 1.97.1 | 1.97.1 |", "| Rust | 9.9.9 | 9.9.9 |"],
    ["| CMake | 4.4.2 | 4.4.2 |", "| CMake | 9.9.9 | 9.9.9 |"],
    ["| Ninja | 1.13.2 | 1.13.2 |", "| Ninja | 9.9.9 | 9.9.9 |"],
  ]) {
    assert.throws(
      () => validatePhase05Acceptance(withMarkdown(input.markdown.replace(before, after))),
      /version manifest mismatch/u,
    );
  }

  const driftedManifest = structuredClone(input.manifest);
  driftedManifest.versions.tools.node.declared = "0.0.0";
  assert.throws(
    () => validatePhase05Acceptance({ ...input, manifest: driftedManifest }),
    /manifest version authority drift/u,
  );

  const driftedInspection = structuredClone(input.manifest);
  driftedInspection.binaries[0].inspections.undefinedSymbols.tool = "/usr/bin/true";
  assert.throws(
    () => validatePhase05Acceptance({ ...input, manifest: driftedInspection }),
    /manifest inspection provenance/u,
  );
});
