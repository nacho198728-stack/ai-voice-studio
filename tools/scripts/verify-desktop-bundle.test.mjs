import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import { verifyDesktopBundle } from "./verify-desktop-bundle.mjs";

test("bundle verification accepts only the expected macOS sidecar and resource layout", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "aivs-bundle-声音-"));
  const app = path.join(root, "AI Voice Studio.app");
  const expected = [
    "Contents/MacOS/ai-voice-studio",
    "Contents/MacOS/voice-runtime",
    "Contents/Resources/native/aivs_mock_voice_engine-aarch64-apple-darwin.dylib",
    "Contents/Resources/config/config.json",
  ];
  for (const relative of expected) {
    const absolute = path.join(app, relative);
    await mkdir(path.dirname(absolute), { recursive: true });
    await writeFile(absolute, relative);
  }

  assert.deepEqual(await verifyDesktopBundle(app, "aarch64-apple-darwin"), {
    application: path.join(app, expected[0]),
    runtime: path.join(app, expected[1]),
    plugin: path.join(app, expected[2]),
    config: path.join(app, expected[3]),
  });
});

test("bundle verification rejects the accidental nested resources layout", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "aivs-bundle-nested-"));
  const app = path.join(root, "AI Voice Studio.app");
  for (const relative of [
    "Contents/MacOS/ai-voice-studio",
    "Contents/MacOS/voice-runtime",
    "Contents/Resources/resources/native/aivs_mock_voice_engine-aarch64-apple-darwin.dylib",
    "Contents/Resources/resources/config/config.json",
  ]) {
    const absolute = path.join(app, relative);
    await mkdir(path.dirname(absolute), { recursive: true });
    await writeFile(absolute, relative);
  }

  await assert.rejects(
    verifyDesktopBundle(app, "aarch64-apple-darwin"),
    /bundle is missing a required Tauri-owned artifact/,
  );
});

test("bundle verification accepts the Windows x64 executable and DLL layout", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "aivs-bundle-windows-声音-"));
  const expected = [
    "ai-voice-studio.exe",
    "voice-runtime.exe",
    "native/aivs_mock_voice_engine-x86_64-pc-windows-msvc.dll",
    "config/config.json",
  ];
  for (const relative of expected) {
    const absolute = path.join(root, relative);
    await mkdir(path.dirname(absolute), { recursive: true });
    await writeFile(absolute, relative);
  }

  assert.deepEqual(await verifyDesktopBundle(root, "x86_64-pc-windows-msvc"), {
    application: path.join(root, expected[0]),
    runtime: path.join(root, expected[1]),
    plugin: path.join(root, expected[2]),
    config: path.join(root, expected[3]),
  });
});
