import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import { nativeArtifactPlan, stageNativeArtifacts } from "./stage-desktop-native.mjs";

test("native artifact plan maps macOS and Windows CMake outputs to target-qualified Tauri names", () => {
  assert.deepEqual(nativeArtifactPlan("Debug", "aarch64-apple-darwin"), {
    runtimeSource: "build/native-debug/bin/Debug/voice-runtime",
    pluginSource: "build/native-debug/lib/Debug/libaivs_mock_voice_engine.dylib",
    runtimeDestination: "apps/desktop/src-tauri/binaries/voice-runtime-aarch64-apple-darwin",
    pluginDestination:
      "apps/desktop/src-tauri/resources/native/aivs_mock_voice_engine-aarch64-apple-darwin.dylib",
    configDestination: "apps/desktop/src-tauri/resources/config/config.json",
  });
  assert.deepEqual(nativeArtifactPlan("Release", "x86_64-pc-windows-msvc"), {
    runtimeSource: "build/native-release/bin/Release/voice-runtime.exe",
    pluginSource: "build/native-release/bin/Release/aivs_mock_voice_engine.dll",
    runtimeDestination:
      "apps/desktop/src-tauri/binaries/voice-runtime-x86_64-pc-windows-msvc.exe",
    pluginDestination:
      "apps/desktop/src-tauri/resources/native/aivs_mock_voice_engine-x86_64-pc-windows-msvc.dll",
    configDestination: "apps/desktop/src-tauri/resources/config/config.json",
  });
});

test("staging copies only the native sidecar, Mock plugin, and validated product config", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "aivs-stage-声音-"));
  const plan = nativeArtifactPlan("Debug", "aarch64-apple-darwin");
  for (const [relative, bytes] of [
    [plan.runtimeSource, "runtime"],
    [plan.pluginSource, "plugin"],
    ["config/config.json", '{"schema_version":1}'],
  ]) {
    const absolute = path.join(root, relative);
    await mkdir(path.dirname(absolute), { recursive: true });
    await writeFile(absolute, bytes);
  }

  const staged = await stageNativeArtifacts({
    repositoryRoot: root,
    profile: "Debug",
    target: "aarch64-apple-darwin",
  });

  assert.deepEqual(staged, {
    runtime: path.join(root, plan.runtimeDestination),
    plugin: path.join(root, plan.pluginDestination),
    config: path.join(root, plan.configDestination),
  });
  assert.equal(await readFile(staged.runtime, "utf8"), "runtime");
  assert.equal(await readFile(staged.plugin, "utf8"), "plugin");
  assert.equal(await readFile(staged.config, "utf8"), '{"schema_version":1}');
});

test("staging rejects traversal-like target triples and missing source artifacts", async () => {
  assert.throws(() => nativeArtifactPlan("Debug", "../../host"), /target triple/);
  await assert.rejects(
    stageNativeArtifacts({
      repositoryRoot: "/definitely/missing/aivs",
      profile: "Debug",
      target: "aarch64-apple-darwin",
    }),
    /Run `pnpm native:configure && pnpm native:build`/,
  );
});
