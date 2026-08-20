import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { parse } from "yaml";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const workflowPath = path.join(repositoryRoot, ".github/workflows/build.yml");

const actionPins = [
  "actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd",
  "actions/setup-node@48b55a011bda9f5d6aeb4c2d9c7362e8dae4041e",
  "lukka/get-cmake@fffaaafeea488556c2c12dad60690008bc1caacb",
];

async function loadWorkflow() {
  return parse(await readFile(workflowPath, "utf8"));
}

function commands(job) {
  return job.steps
    .filter((step) => typeof step.run === "string")
    .map((step) => step.run)
    .join("\n");
}

function actionStep(job, prefix) {
  return job.steps.find((step) => step.uses?.startsWith(prefix));
}

test("workflow routes only read-only macOS arm64 and Windows x64 build jobs", async () => {
  const workflow = await loadWorkflow();
  assert.deepEqual(Object.keys(workflow.on).sort(), ["pull_request", "push", "workflow_dispatch"]);
  assert.deepEqual(workflow.permissions, { contents: "read" });
  assert.deepEqual(Object.keys(workflow.jobs).sort(), ["macos-arm64", "windows-x64"]);
  assert.equal(workflow.jobs["macos-arm64"]["runs-on"], "macos-15");
  assert.equal(workflow.jobs["windows-x64"]["runs-on"], "windows-latest");
  assert.equal(workflow.jobs["macos-arm64"]["timeout-minutes"], 45);
  assert.equal(workflow.jobs["windows-x64"]["timeout-minutes"], 45);
});

test("workflow pins reviewed actions and exact tools without cache correctness", async () => {
  const workflow = await loadWorkflow();
  for (const job of Object.values(workflow.jobs)) {
    assert.deepEqual(
      job.steps.filter((step) => step.uses).map((step) => step.uses),
      actionPins,
    );
    assert.equal(actionStep(job, "actions/setup-node@").with["node-version-file"], ".node-version");
    assert.equal(actionStep(job, "actions/setup-node@").with["package-manager-cache"], false);
    const cmake = actionStep(job, "lukka/get-cmake@");
    assert.equal(cmake.with.cmakeVersion, "4.4.2");
    assert.equal(cmake.with.ninjaVersion, "1.13.2");
    assert.equal(cmake.with.useCloudCache, false);
    assert.equal(cmake.with.useLocalCache, false);
    assert.doesNotMatch(job.steps.map((step) => step.uses ?? "").join("\n"), /@(main|master|v\d+)$/mu);
  }
});

test("both jobs execute the locked workspace and Debug plus Release contracts", async () => {
  const workflow = await loadWorkflow();
  const required = [
    /corepack prepare pnpm@11\.19\.0 --activate/u,
    /rustup toolchain install 1\.97\.1/u,
    /pnpm install --frozen-lockfile/u,
    /pnpm contracts:check/u,
    /pnpm test/u,
    /pnpm --filter @ai-voice-studio\/desktop lint/u,
    /pnpm --filter @ai-voice-studio\/desktop typecheck/u,
    /pnpm --filter @ai-voice-studio\/desktop build/u,
    /cargo \+1\.97\.1 fmt --all -- --check/u,
    /cargo \+1\.97\.1 check --locked --workspace --all-targets/u,
    /cargo \+1\.97\.1 clippy --locked --workspace --all-targets -- -D warnings/u,
    /cargo \+1\.97\.1 test --locked --workspace/u,
    /cmake --preset native-debug/u,
    /cmake --build --preset native-debug/u,
    /ctest --preset native-debug/u,
    /cmake --preset native-release/u,
    /cmake --build --preset native-release/u,
    /ctest --preset native-release/u,
  ];
  for (const job of Object.values(workflow.jobs)) {
    const run = commands(job);
    for (const expected of required) assert.match(run, expected);
  }
});

test("both jobs build and stage native artifacts, warm Rust, then execute CTest", async () => {
  const workflow = await loadWorkflow();
  for (const job of Object.values(workflow.jobs)) {
    const run = commands(job);
    const nativeBuildIndex = run.indexOf("cmake --build --preset native-release");
    const stageIndex = run.indexOf("stage-desktop-native.mjs --profile Release");
    const rustCheckIndex = run.indexOf("cargo +1.97.1 check --locked --workspace --all-targets");
    const ctestWarmupIndex = run.indexOf("cargo +1.97.1 test --locked --workspace --no-run");
    const nativeTestIndex = run.indexOf("ctest --preset native-release");
    assert.notEqual(nativeBuildIndex, -1);
    assert.notEqual(stageIndex, -1);
    assert.notEqual(rustCheckIndex, -1);
    assert.notEqual(ctestWarmupIndex, -1);
    assert.notEqual(nativeTestIndex, -1);
    assert.match(run, /build\/native-debug\/rust-target/u);
    assert.match(run, /build\/native-release\/rust-target/u);
    assert.ok(
      nativeBuildIndex < stageIndex
        && stageIndex < rustCheckIndex
        && rustCheckIndex < ctestWarmupIndex
        && ctestWarmupIndex < nativeTestIndex,
      "native binaries must be staged before Cargo, while Cargo must warm fixture tests before CTest",
    );
  }
});

test("platform jobs fail closed on architecture and exercise native Tauri layouts", async () => {
  const workflow = await loadWorkflow();
  const macos = commands(workflow.jobs["macos-arm64"]);
  assert.match(macos, /test "\$\(uname -m\)" = "arm64"/u);
  assert.match(macos, /aarch64-apple-darwin/u);
  assert.match(macos, /stage-desktop-native\.mjs --profile Release --target aarch64-apple-darwin/u);
  assert.match(macos, /pnpm desktop:test:native/u);
  assert.match(macos, /tauri build --bundles app/u);
  assert.match(macos, /target\/release\/bundle\/macos\/AI Voice Studio\.app/u);

  const windows = commands(workflow.jobs["windows-x64"]);
  assert.match(windows, /OSArchitecture.*X64/u);
  assert.match(windows, /x86_64-pc-windows-msvc/u);
  assert.match(windows, /Enter-VsDevShell/u);
  assert.match(windows, /-arch=x64 -host_arch=x64/u);
  for (const gate of [
    "aivs_mock_voice_engine_exports",
    "aivs_mock_voice_engine_dynamic_test",
    "aivs_voice_runtime_smoke",
    "aivs_runtime_manager_child_test",
    "aivs_runtime_manager_gate_test",
    "aivs_desktop_exit_runtime_race_test",
  ]) {
    assert.match(windows, new RegExp(gate, "u"));
  }
  assert.match(windows, /stage-desktop-native\.mjs --profile Release --target x86_64-pc-windows-msvc/u);
  assert.match(windows, /pnpm desktop:test:native/u);
  assert.match(windows, /tauri build --no-bundle/u);
  assert.match(windows, /voice-runtime-x86_64-pc-windows-msvc\.exe/u);
  assert.match(windows, /aivs_mock_voice_engine-x86_64-pc-windows-msvc\.dll/u);
  assert.match(windows, /verify-desktop-bundle\.mjs.*x86_64-pc-windows-msvc/u);

  const windowsIcon = await readFile(
    path.join(repositoryRoot, "apps/desktop/src-tauri/icons/icon.ico"),
  );
  assert.deepEqual(
    [...windowsIcon.subarray(0, 4)],
    [0, 0, 1, 0],
    "the clean Windows Tauri build requires a real ICO resource",
  );
});
