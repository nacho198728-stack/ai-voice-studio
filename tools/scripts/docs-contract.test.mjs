import assert from "node:assert/strict";
import { access, readdir, readFile, stat } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { parse as parseYaml } from "yaml";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const requiredDocuments = [
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
const sourceDocumentationRoots = [
  "apps",
  "audio",
  "backend",
  "config",
  "core",
  "engines",
  "runtime",
  "tests",
  "tools",
];
const excludedGeneratedDirectories = new Set([
  "binaries",
  "build",
  "dist",
  "gen",
  "node_modules",
  "resources",
  "target",
]);
const expectedChainEdges = [
  ["Tauri webview", "Desktop command allowlist"],
  ["Desktop command allowlist", "CommandService / RuntimeManager"],
  ["CommandService / RuntimeManager", "RuntimeMessage v1"],
  ["RuntimeMessage v1", "voice-runtime Session"],
  ["voice-runtime Session", "MockPipeline"],
  ["MockPipeline", "VoiceEngine loader"],
  ["VoiceEngine loader", "VoiceEngine C ABI"],
  ["VoiceEngine C ABI", "Mock VoiceEngine"],
];

async function text(relative) {
  return readFile(path.join(repositoryRoot, relative), "utf8");
}

async function filesBelow(relativeDirectory, suffix, excludedDirectories = new Set()) {
  const root = path.join(repositoryRoot, relativeDirectory);
  const found = [];
  async function visit(directory) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const candidate = path.join(directory, entry.name);
      if (entry.isDirectory() && !excludedDirectories.has(entry.name)) await visit(candidate);
      else if (entry.isFile() && entry.name.endsWith(suffix)) {
        found.push(path.relative(repositoryRoot, candidate));
      }
    }
  }
  await visit(root);
  return found.sort();
}

function tableAfterHeading(markdown, heading) {
  const lines = markdown.split(/\r?\n/u);
  const headingIndex = lines.findIndex((line) => line.trim() === heading);
  if (headingIndex < 0) return [];
  let index = headingIndex + 1;
  while (
    index < lines.length &&
    !lines[index].trim().startsWith("|") &&
    !lines[index].match(/^#{1,6}\s/u)
  ) {
    index += 1;
  }
  if (!lines[index]?.trim().startsWith("|") || !lines[index + 1]?.match(/^\s*\|[\s:|-]+\|\s*$/u)) {
    return [];
  }
  const rows = [];
  for (index += 2; index < lines.length && lines[index].trim().startsWith("|"); index += 1) {
    rows.push(
      lines[index]
        .trim()
        .slice(1, -1)
        .split("|")
        .map((cell) => cell.trim()),
    );
  }
  return rows;
}

function replaceTableRows(markdown, heading, rows) {
  const lines = markdown.split(/\r?\n/u);
  const headingIndex = lines.findIndex((line) => line.trim() === heading);
  let firstRow = headingIndex + 1;
  while (firstRow < lines.length && !lines[firstRow].trim().startsWith("|")) firstRow += 1;
  firstRow += 2;
  let afterRows = firstRow;
  while (afterRows < lines.length && lines[afterRows].trim().startsWith("|")) afterRows += 1;
  return [
    ...lines.slice(0, firstRow),
    ...rows.map((row) => `| ${row.join(" | ")} |`),
    ...lines.slice(afterRows),
  ].join("\n");
}

function architectureChainIsOrdered(markdown) {
  const edges = tableAfterHeading(markdown, "### Ordered process and plugin edges").map((row) =>
    row.slice(0, 2),
  );
  return JSON.stringify(edges) === JSON.stringify(expectedChainEdges);
}

function rustStructFields(source, name) {
  const body = source.match(new RegExp(`pub struct ${name} \\{([\\s\\S]*?)^\\}`, "mu"))?.[1];
  assert.ok(body, `missing Rust struct ${name}`);
  return [...body.matchAll(/^\s*pub\s+(\w+)\s*:/gmu)].map((match) => match[1]);
}

function tauriCommandAllowlist(source) {
  const body = source.match(/tauri::generate_handler!\[([\s\S]*?)\]/u)?.[1];
  assert.ok(body, "missing Tauri generate_handler allowlist");
  return body
    .split(",")
    .map((command) => command.trim())
    .filter(Boolean);
}

function tableFirstColumn(markdown, heading) {
  return tableAfterHeading(markdown, heading).map((row) => row[0].replaceAll("`", ""));
}

function isReservedAdr003Name(name) {
  return /^adr[-_ ]?0*3(?:[-_. ]|$)/iu.test(name);
}

function stripCode(value) {
  return value.replaceAll("`", "").trim();
}

function threePartVersion(value) {
  return value.split(".").length === 2 ? `${value}.0` : value;
}

function tableRowsByKey(markdown, heading) {
  return new Map(
    tableAfterHeading(markdown, heading).map((row) => [stripCode(row[0]), row.map(stripCode)]),
  );
}

function sourceObjectVersion(source, objectName, key) {
  const body = source.match(new RegExp(`const ${objectName} = \\{([\\s\\S]*?)\\n\\};`, "u"))?.[1];
  const value = body?.match(new RegExp(`^\\s*${key}: ['\"]([^'\"]+)['\"]`, "mu"))?.[1];
  assert.ok(value, `missing ${objectName}.${key}`);
  return value.split(".").length === 2 ? `${value}.0` : value;
}

function assertVersionInRow(rows, tool, expectedMinimum, expectedValidated, expectedAuthority) {
  const row = rows.get(tool);
  assert.ok(row, `missing toolchain table row ${tool}`);
  assert.ok(row[1].includes(expectedMinimum), `${tool} row has the wrong required/minimum version`);
  assert.ok(row[2].includes(expectedValidated), `${tool} row has the wrong validated/authority value`);
  if (expectedAuthority) {
    assert.ok(row[2].includes(expectedAuthority), `${tool} row is not bound to ${expectedAuthority}`);
  }
}

function shellBlocks(markdown) {
  return [...markdown.matchAll(/```(?:sh|bash|powershell|pwsh)\n([\s\S]*?)```/gu)]
    .map((match) => match[1])
    .join("\n");
}

function commandLines(markdown) {
  return shellBlocks(markdown)
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .filter((line) => line && !line.startsWith("#"));
}

function markdownTargets(markdown) {
  const definitions = new Map(
    [...markdown.matchAll(/^\s*\[([^\]]+)\]:\s*(\S+)/gmu)].map((match) => [
      match[1].toLowerCase(),
      match[2],
    ]),
  );
  const targets = [...markdown.matchAll(/!?\[[^\]]*\]\(([^)]+)\)/gu)].map((match) => match[1]);
  for (const match of markdown.matchAll(/!?\[[^\]]+\]\[([^\]]+)\]/gu)) {
    assert.ok(definitions.has(match[1].toLowerCase()), `missing Markdown reference ${match[1]}`);
  }
  targets.push(...definitions.values());
  return targets;
}

function cleanMarkdownTarget(rawTarget) {
  const target = rawTarget.trim().replace(/^<|>$/gu, "");
  return target.match(/^(?:<([^>]+)>|([^\s]+))/u)?.slice(1).find(Boolean) ?? target;
}

function headingAnchors(markdown) {
  const counts = new Map();
  const anchors = new Set();
  for (const match of markdown.matchAll(/^#{1,6}\s+(.+?)\s*#*$/gmu)) {
    const base = match[1]
      .toLowerCase()
      .replace(/[`*_~]/gu, "")
      .replace(/[^\p{Letter}\p{Number}\s-]/gu, "")
      .trim()
      .replace(/\s+/gu, "-");
    const count = counts.get(base) ?? 0;
    counts.set(base, count + 1);
    anchors.add(count === 0 ? base : `${base}-${count}`);
  }
  return anchors;
}

async function assertMarkdownLinkResolves(document, rawTarget) {
  const target = cleanMarkdownTarget(rawTarget);
  if (!target || /^(?:https?:|mailto:)/iu.test(target)) return;
  const [encodedPath, encodedAnchor] = target.split("#", 2);
  const relativeTarget = decodeURIComponent(encodedPath);
  const anchor = encodedAnchor ? decodeURIComponent(encodedAnchor).toLowerCase() : "";
  const absoluteTarget = relativeTarget
    ? path.resolve(path.dirname(path.join(repositoryRoot, document)), relativeTarget)
    : path.join(repositoryRoot, document);
  assert.ok(absoluteTarget.startsWith(repositoryRoot + path.sep), `${document} links outside repository`);
  await access(absoluteTarget);
  if (!anchor || (await stat(absoluteTarget)).isDirectory()) return;
  const targetMarkdown = await readFile(absoluteTarget, "utf8");
  assert.ok(headingAnchors(targetMarkdown).has(anchor), `${document} has unresolved anchor ${target}`);
}

function windowsEvidenceIsAccepted(adr) {
  const rows = tableRowsByKey(adr, "### Platform verification status");
  const windows = rows.get("Windows x64");
  if (!windows) return false;
  const status = windows.slice(1).join(" ").toLowerCase();
  return (
    status.includes("accepted") &&
    status.includes("real github-hosted windows x64/msvc") &&
    status.includes("task 18") &&
    !status.includes("pending")
  );
}

test("ordered architecture validator rejects a fully reversed chain mutation", async () => {
  const architecture = await text("docs/architecture/ARCHITECTURE.md");
  assert.equal(architectureChainIsOrdered(architecture), true, "checked-in chain must be ordered");
  const reversedEdges = [...expectedChainEdges].reverse().map(([from, to]) => [to, from]);
  const reversed = replaceTableRows(
    architecture,
    "### Ordered process and plugin edges",
    reversedEdges.map(([from, to]) => [from, to, "mutated reverse edge"]),
  );
  assert.equal(architectureChainIsOrdered(reversed), false);
});

test("required ADR history is complete and every ADR-003 filename variant is reserved", async () => {
  for (const document of requiredDocuments) await access(path.join(repositoryRoot, document));
  for (const name of ["ADR-003.md", "adr_003-audio.md", "ADR 0003 draft.md"]) {
    assert.equal(isReservedAdr003Name(name), true, `reservation matcher missed ${name}`);
  }
  const adrFiles = await filesBelow("docs/adr", "");
  assert.deepEqual(adrFiles.filter((file) => isReservedAdr003Name(path.basename(file))), []);

  for (const adr of requiredDocuments.filter((document) => document.includes("/ADR-"))) {
    const markdown = await text(adr);
    assert.match(markdown, /^- Status: Accepted$/mu, `${adr} must record Accepted status`);
    for (const heading of ["Context", "Decision", "Rationale", "Alternatives considered", "Consequences"]) {
      assert.match(markdown, new RegExp(`^## ${heading}$`, "imu"), `${adr} is missing ${heading}`);
    }
  }
});

test("links resolve across docs and every in-scope repository README", async () => {
  // Plans/task evidence are separate governance records. Generated, staged, build, dependency,
  // and Cargo target trees are excluded; all authored docs and source-tree READMEs are included.
  const sourceReadmes = (
    await Promise.all(
      sourceDocumentationRoots.map((root) =>
        filesBelow(root, "README.md", excludedGeneratedDirectories),
      ),
    )
  ).flat();
  const documents = [...(await filesBelow("docs", ".md")), "README.md", ...sourceReadmes];
  assert.equal(new Set(documents).size, documents.length, "link scope must not contain duplicates");
  for (const document of documents) {
    const markdown = await text(document);
    for (const target of markdownTargets(markdown)) {
      await assertMarkdownLinkResolves(document, target);
    }
  }

  const referenceFixture =
    "[Development guide][development]\n\n[development]: <docs/development/DEVELOPMENT.md#toolchain-contract>\n";
  const [referenceTarget] = markdownTargets(referenceFixture);
  assert.equal(referenceTarget, "<docs/development/DEVELOPMENT.md#toolchain-contract>");
  await assertMarkdownLinkResolves("README.md", referenceTarget);
  await assert.rejects(
    assertMarkdownLinkResolves(
      "README.md",
      "docs/development/DEVELOPMENT.md#missing-documentation-anchor",
    ),
  );
});

test("tool versions are bound to their authoritative DEVELOPMENT table rows", async () => {
  const development = await text("docs/development/DEVELOPMENT.md");
  const exactRows = tableRowsByKey(development, "### Exact repository tools");
  const nativeRows = tableRowsByKey(development, "### Native minimums and validated versions");
  const rootPackage = JSON.parse(await text("package.json"));
  const desktopPackage = JSON.parse(await text("apps/desktop/package.json"));
  const rustToolchain = await text("rust-toolchain.toml");
  const workflow = parseYaml(await text(".github/workflows/build.yml"));
  const doctor = await text("tools/scripts/doctor.mjs");
  const nodeVersion = (await text(".node-version")).trim();
  const pnpmVersion = rootPackage.packageManager.split("@")[1];
  const rustVersion = rustToolchain.match(/channel = "([^"]+)"/u)?.[1];
  const cmakeMinimum = (await text("CMakeLists.txt")).match(/cmake_minimum_required\(VERSION ([^)]+)\)/u)?.[1];
  const setup = workflow.jobs["macos-arm64"].steps.find((step) => step.uses?.startsWith("lukka/get-cmake@"));

  assertVersionInRow(exactRows, "Node.js", nodeVersion, ".node-version", ".node-version");
  assertVersionInRow(exactRows, "pnpm", pnpmVersion, "root packageManager", "packageManager");
  assertVersionInRow(exactRows, "Rust", rustVersion, "rust-toolchain.toml", "rust-toolchain.toml");
  assertVersionInRow(
    exactRows,
    "Tauri CLI",
    desktopPackage.devDependencies["@tauri-apps/cli"],
    "apps/desktop/package.json",
    "apps/desktop/package.json",
  );
  assertVersionInRow(nativeRows, "CMake", threePartVersion(cmakeMinimum), setup.with.cmakeVersion);
  assertVersionInRow(
    nativeRows,
    "Ninja",
    sourceObjectVersion(doctor, "minimumVersions", "ninja"),
    setup.with.ninjaVersion,
  );
  assertVersionInRow(
    nativeRows,
    "Git",
    sourceObjectVersion(doctor, "minimumVersions", "git"),
    sourceObjectVersion(doctor, "validatedVersions", "git"),
  );
  assertVersionInRow(
    nativeRows,
    "Xcode",
    sourceObjectVersion(doctor, "minimumVersions", "xcode"),
    sourceObjectVersion(doctor, "validatedVersions", "xcode"),
  );
  assertVersionInRow(
    nativeRows,
    "Visual Studio / MSVC",
    sourceObjectVersion(doctor, "minimumVersions", "msvc"),
    "accepted real Windows x64 workflow run",
  );

  const wrongRowWithStrayCorrectToken = development
    .replace(`| Node.js | \`${nodeVersion}\` |`, "| Node.js | `0.0.0` |")
    .concat(`\nUnrelated token: \`${nodeVersion}\`.\n`);
  const mutatedRows = tableRowsByKey(wrongRowWithStrayCorrectToken, "### Exact repository tools");
  assert.throws(() =>
    assertVersionInRow(mutatedRows, "Node.js", nodeVersion, ".node-version", ".node-version"),
  );
});

test("documented commands resolve to real scripts, packages, presets, targets, and CI layout", async () => {
  const development = await text("docs/development/DEVELOPMENT.md");
  const lines = commandLines(development);
  const rootPackage = JSON.parse(await text("package.json"));
  const desktopPackage = JSON.parse(await text("apps/desktop/package.json"));
  const packages = new Map([[desktopPackage.name, desktopPackage]]);
  const presetDocument = JSON.parse(await text("CMakePresets.json"));
  const configurePresets = new Set(presetDocument.configurePresets.map((preset) => preset.name));
  const buildPresets = new Set(presetDocument.buildPresets.map((preset) => preset.name));
  const testPresets = new Set(presetDocument.testPresets.map((preset) => preset.name));
  const rustVersion = (await text("rust-toolchain.toml")).match(/channel = "([^"]+)"/u)?.[1];
  const workflow = parseYaml(await text(".github/workflows/build.yml"));
  const workflowCommands = Object.values(workflow.jobs)
    .flatMap((job) => job.steps)
    .flatMap((step) => (typeof step.run === "string" ? step.run.split(/\r?\n/u) : []))
    .map((line) => line.trim());

  for (const line of lines) {
    const filtered = line.match(/^pnpm --filter (\S+) (\S+)/u);
    if (filtered) {
      const packageDocument = packages.get(filtered[1]);
      assert.ok(packageDocument, `unknown documented workspace package ${filtered[1]}`);
      assert.ok(packageDocument.scripts[filtered[2]], `unknown ${filtered[1]} script ${filtered[2]}`);
      continue;
    }
    const rootPnpm = line.match(/^pnpm (?:run )?([\w:-]+)/u);
    if (rootPnpm && rootPnpm[1] !== "install") {
      assert.ok(rootPackage.scripts[rootPnpm[1]], `unknown documented root script ${rootPnpm[1]}`);
    }
    const nodeScript = line.match(/^node ([^\s]+\.mjs)(?:\s|$)/u);
    if (nodeScript) await access(path.join(repositoryRoot, nodeScript[1]));
    const configure = line.match(/^cmake --preset ([\w-]+)/u);
    if (configure) assert.ok(configurePresets.has(configure[1]), `unknown configure preset ${configure[1]}`);
    const build = line.match(/^cmake --build --preset ([\w-]+)/u);
    if (build) assert.ok(buildPresets.has(build[1]), `unknown build preset ${build[1]}`);
    const ctest = line.match(/^ctest --preset ([\w-]+)/u);
    if (ctest) assert.ok(testPresets.has(ctest[1]), `unknown test preset ${ctest[1]}`);
    const cargo = line.match(/^cargo \+(\S+)/u);
    if (cargo) assert.equal(cargo[1], rustVersion, "documented Cargo toolchain drifted");
    const rustup = line.match(/^rustup toolchain install (\S+)/u);
    if (rustup) assert.equal(rustup[1], rustVersion, "documented rustup toolchain drifted");
    const corepack = line.match(/^corepack prepare pnpm@(\S+) --activate$/u);
    if (corepack) {
      assert.equal(corepack[1], rootPackage.packageManager.split("@")[1], "documented pnpm pin drifted");
    }
  }

  for (const line of lines.filter((candidate) => candidate.startsWith("Copy-Item "))) {
    assert.ok(workflowCommands.includes(line), `Windows layout command drifted from CI: ${line}`);
  }
  for (const line of lines.filter((candidate) =>
    /stage-desktop-native\.mjs.*--target|tauri build .*--ci/u.test(candidate),
  )) {
    assert.ok(workflowCommands.includes(line), `staging/Tauri arguments drifted from CI: ${line}`);
  }
});

test("architecture graph, commands, and public DTO tables match implementation sources", async () => {
  const architecture = await text("docs/architecture/ARCHITECTURE.md");
  const desktopSource = await text("apps/desktop/src-tauri/src/lib.rs");
  const managerSource = await text("apps/runtime-host/src/manager.rs");
  const stdioSource = await text("runtime/process/stdio_runtime.cpp");
  const pipelineSource = await text("runtime/pipeline/mock_pipeline.cpp");
  const loaderSource = await text("runtime/loader/voice_engine_loader.cpp");
  const mockSource = await text("engines/mock/mock_voice_engine.cpp");
  const payloadSource = await text("apps/runtime-host/src/payload.rs");

  assert.equal(architectureChainIsOrdered(architecture), true);
  assert.deepEqual(
    tableFirstColumn(architecture, "### Public command allowlist"),
    tauriCommandAllowlist(desktopSource),
  );
  assert.deepEqual(
    tableFirstColumn(architecture, "### CapabilityDto public fields"),
    rustStructFields(desktopSource, "CapabilityDto"),
  );
  assert.deepEqual(
    tableFirstColumn(architecture, "### MockPipelineSummaryDto public fields"),
    rustStructFields(desktopSource, "MockPipelineSummaryDto"),
  );
  const capabilityRows = tableRowsByKey(architecture, "### CapabilityDto public fields");
  assert.ok(capabilityRows.get("runtime_availability")?.join(" ").includes("not_evaluated"));
  assert.ok(capabilityRows.get("engine_availability")?.join(" ").includes("not_evaluated"));

  assert.match(managerSource, /RuntimeMessage/u);
  assert.match(stdioSource, /Session session\(generation, pipeline\)/u);
  assert.match(stdioSource, /MockPipeline::create/u);
  assert.match(pipelineSource, /VoiceEngineModule::load\(plugin_path\)/u);
  assert.match(loaderSource, /aivs_voice_engine_get_api/u);
  assert.match(mockSource, /aivs_voice_engine_get_api\(/u);
  const summaryBytes = payloadSource.match(/const MOCK_SUMMARY_BYTES: usize = (\d+);/u)?.[1];
  assert.ok(summaryBytes && architecture.includes(`${summaryBytes}-byte`));
  assert.match(desktopSource, /CapabilityAvailability::NotEvaluated => "not_evaluated"/u);
});

test("Windows ABI evidence records the accepted real runner", async () => {
  const adr = await text("docs/adr/ADR-002-cpp-runtime-c-abi.md");
  const workflow = parseYaml(await text(".github/workflows/build.yml"));
  assert.ok(workflow.jobs["windows-x64"], "Windows validation path must remain configured");
  assert.equal(windowsEvidenceIsAccepted(adr), true);
  const reverted = adr.replace(
    "Accepted: real GitHub-hosted Windows x64/MSVC workflow passed Task 18.",
    "Source/config/fixture only; Task 18 pending and not accepted.",
  );
  assert.equal(windowsEvidenceIsAccepted(reverted), false);
});

test("architecture records scope exclusions, summary wire size, and ADR-003 reservation", async () => {
  const architecture = await text("docs/architecture/ARCHITECTURE.md");
  for (const exclusion of ["no audio device", "no ai", "no python"]) {
    assert.ok(architecture.toLowerCase().includes(exclusion), `architecture is missing ${exclusion}`);
  }
  assert.match(architecture, /80-byte/u);
  assert.match(architecture, /ADR-003/u);
});
