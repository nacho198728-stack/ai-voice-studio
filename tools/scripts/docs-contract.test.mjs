import assert from "node:assert/strict";
import { access, readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { parse as parseYaml } from "yaml";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const documents = [
  "docs/README.md",
  "docs/development/DEVELOPMENT.md",
  "docs/development/CONFIGURATION.md",
  "docs/architecture/ARCHITECTURE.md",
  "docs/contracts/voice-runtime-stdio-v1.md",
  "docs/contracts/voice-engine-c-abi-v1.md",
  "docs/adr/ADR-000-monorepo.md",
  "docs/adr/ADR-001-tauri-rust-control-plane.md",
  "docs/adr/ADR-002-cpp-runtime-c-abi.md",
];

async function text(relative) {
  return readFile(path.join(repositoryRoot, relative), "utf8");
}

function shellBlocks(markdown) {
  return [...markdown.matchAll(/```(?:sh|bash|powershell|pwsh)\n([\s\S]*?)```/gu)]
    .map((match) => match[1])
    .join("\n");
}

function threePartVersion(version) {
  return version.split(".").length === 2 ? `${version}.0` : version;
}

test("required architecture and accepted ADR history have complete sections", async () => {
  for (const document of documents) await access(path.join(repositoryRoot, document));
  await assert.rejects(access(path.join(repositoryRoot, "docs/adr/ADR-003-audio-engine.md")));

  for (const adr of documents.filter((document) => document.includes("/ADR-"))) {
    const markdown = await text(adr);
    assert.match(markdown, /^- Status: Accepted$/mu, `${adr} must record Accepted status`);
    for (const heading of ["Context", "Decision", "Rationale", "Alternatives considered", "Consequences"]) {
      assert.match(markdown, new RegExp(`^## ${heading}$`, "imu"), `${adr} is missing ${heading}`);
    }
  }
});

test("every repository-relative Markdown link resolves", async () => {
  for (const document of documents) {
    const markdown = await text(document);
    const links = [...markdown.matchAll(/!?\[[^\]]*\]\(([^)]+)\)/gu)].map((match) => match[1]);
    for (const rawLink of links) {
      const link = rawLink.trim().replace(/^<|>$/gu, "").split(/[ \t]+["']/u, 1)[0];
      if (!link || /^(?:https?:|mailto:|#)/u.test(link)) continue;
      const relativeTarget = decodeURIComponent(link.split("#", 1)[0]);
      const absoluteTarget = path.resolve(path.dirname(path.join(repositoryRoot, document)), relativeTarget);
      await access(absoluteTarget);
    }
  }
});

test("developer guide versions follow repository and CI declarations", async () => {
  const development = await text("docs/development/DEVELOPMENT.md");
  const nodeVersion = (await text(".node-version")).trim();
  const packageDocument = JSON.parse(await text("package.json"));
  const pnpmVersion = packageDocument.packageManager.split("@")[1];
  const rustVersion = (await text("rust-toolchain.toml")).match(/channel = "([^"]+)"/u)?.[1];
  const cmakeMinimum = (await text("CMakeLists.txt")).match(/cmake_minimum_required\(VERSION ([^)]+)\)/u)?.[1];
  const workflow = parseYaml(await text(".github/workflows/build.yml"));
  const setup = workflow.jobs["macos-arm64"].steps.find((step) => step.uses?.startsWith("lukka/get-cmake@"));
  const tauriCliVersion = JSON.parse(await text("apps/desktop/package.json")).devDependencies[
    "@tauri-apps/cli"
  ];

  for (const version of [
    nodeVersion,
    pnpmVersion,
    rustVersion,
    threePartVersion(cmakeMinimum),
    setup.with.cmakeVersion,
    setup.with.ninjaVersion,
    tauriCliVersion,
  ]) {
    assert.ok(version && development.includes(`\`${version}\``), `missing documented version ${version}`);
  }
});

test("documented pnpm scripts and CMake presets exist", async () => {
  const development = await text("docs/development/DEVELOPMENT.md");
  const commands = shellBlocks(development);
  const rootPackage = JSON.parse(await text("package.json"));
  const presetDocument = JSON.parse(await text("CMakePresets.json"));
  const presetNames = new Set([
    ...presetDocument.configurePresets.map((preset) => preset.name),
    ...presetDocument.buildPresets.map((preset) => preset.name),
    ...presetDocument.testPresets.map((preset) => preset.name),
  ]);

  for (const match of commands.matchAll(/^pnpm (?!install|--filter)(?:run )?([\w:-]+)/gmu)) {
    assert.ok(rootPackage.scripts[match[1]], `unknown documented pnpm script: ${match[1]}`);
  }
  for (const match of commands.matchAll(/--preset ([\w-]+)/gu)) {
    assert.ok(presetNames.has(match[1]), `unknown documented CMake preset: ${match[1]}`);
  }
});

test("architecture states the real chain, exclusions, reservation, and Windows limit", async () => {
  const architecture = await text("docs/architecture/ARCHITECTURE.md");
  const development = await text("docs/development/DEVELOPMENT.md");
  for (const fact of [
    "Tauri",
    "RuntimeManager",
    "RuntimeMessage",
    "voice-runtime",
    "VoiceEngine C ABI",
    "Mock VoiceEngine",
    "80-byte",
    "ADR-003",
  ]) {
    assert.ok(architecture.includes(fact), `architecture is missing ${fact}`);
  }
  for (const exclusion of ["no audio device", "no ai", "no python"]) {
    assert.ok(architecture.toLowerCase().includes(exclusion), `architecture is missing ${exclusion}`);
  }
  for (const markdown of [architecture, development]) {
    assert.match(markdown, /Windows[^\n]*(?:not yet|pending|not accepted)/iu);
  }
});
