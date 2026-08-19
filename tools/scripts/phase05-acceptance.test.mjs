import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { validatePhase05Acceptance } from "./phase05-acceptance.mjs";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

async function repositoryAcceptance() {
  return validatePhase05Acceptance({
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
}

test("checked-in acceptance remains pending only on an unrun real Windows job", async () => {
  const result = await repositoryAcceptance();
  assert.equal(result.status, "PENDING");
  assert.equal(result.windowsEvidenceUrl, null);
  assert.equal(result.requirementCount, 13);
  assert.equal(result.artifactCount, 6);
});

test("macOS absolute evidence remains auditable from a different checkout path", async () => {
  const report = await readFile(
    path.join(repositoryRoot, "docs/development/PHASE-0.5-ACCEPTANCE.md"),
    "utf8",
  );
  const plan = await readFile(
    path.join(repositoryRoot, "plans/2026-08-19-phase-0-5-infrastructure-v1.md"),
    "utf8",
  );
  const result = validatePhase05Acceptance({
    repositoryRoot: "/github/workspace/other-checkout",
    markdown: report,
    planMarkdown: plan,
  });
  assert.equal(result.artifactCount, 6);
});

test("acceptance gate rejects a false Windows result or prematurely checked final plan tasks", async () => {
  const report = await readFile(
    path.join(repositoryRoot, "docs/development/PHASE-0.5-ACCEPTANCE.md"),
    "utf8",
  );
  const plan = await readFile(
    path.join(repositoryRoot, "plans/2026-08-19-phase-0-5-infrastructure-v1.md"),
    "utf8",
  );

  assert.throws(
    () =>
      validatePhase05Acceptance({
        repositoryRoot,
        markdown: report.replace(
          "| Windows x64 | PENDING | PENDING — no run URL available |",
          "| Windows x64 | PASS | https://example.invalid/fake-run |",
        ),
        planMarkdown: plan,
      }),
    /Windows x64 evidence must remain pending/u,
  );
  assert.throws(
    () =>
      validatePhase05Acceptance({
        repositoryRoot,
        markdown: report,
        planMarkdown: plan.replace(
          "- [ ] 在 `.github/workflows/build.yml`",
          "- [x] 在 `.github/workflows/build.yml`",
        ),
      }),
    /Task 18 must remain unchecked/u,
  );
});

test("acceptance gate rejects an incomplete matrix and artifact path drift", async () => {
  const report = await readFile(
    path.join(repositoryRoot, "docs/development/PHASE-0.5-ACCEPTANCE.md"),
    "utf8",
  );
  const plan = await readFile(
    path.join(repositoryRoot, "plans/2026-08-19-phase-0-5-infrastructure-v1.md"),
    "utf8",
  );

  assert.throws(
    () =>
      validatePhase05Acceptance({
        repositoryRoot,
        markdown: report.replace(/^\| EXCL \|.*\n/mu, ""),
        planMarkdown: plan,
      }),
    /verification matrix/u,
  );
  assert.throws(
    () =>
      validatePhase05Acceptance({
        repositoryRoot,
        markdown: report.replace(
          "/Users/alex/Documents/ChatGPT/AI翻唱/target/release/bundle/macos/AI Voice Studio.app/Contents/Info.plist",
          "/tmp/other.app/Contents/Info.plist",
        ),
        planMarkdown: plan,
      }),
    /artifact absolute path/u,
  );
});
