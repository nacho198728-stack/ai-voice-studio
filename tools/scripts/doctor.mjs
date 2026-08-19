#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

const minimumVersions = {
  cmake: '3.25.0',
  ninja: '1.10.0',
  git: '2.40.0',
  xcode: '15.0.0',
  msvc: '19.38.0',
};

const validatedVersions = {
  cmake: '4.4.2',
  ninja: '1.13.2',
  git: '2.50.1',
  xcode: '26.6.0',
};

function defaultRunCommand(command, args) {
  const result = spawnSync(command, args, {
    encoding: 'utf8',
    shell: false,
  });

  if (result.error?.code === 'ENOENT') {
    return { found: false, stdout: '', stderr: '', exitCode: null };
  }

  return {
    found: !result.error,
    stdout: result.stdout ?? '',
    stderr: result.stderr ?? '',
    exitCode: result.status,
  };
}

function parseVersion(value) {
  const match = value.match(/\d+\.\d+(?:\.\d+)?/);
  if (!match) return null;

  return match[0].split('.').map(Number);
}

function compareVersions(left, right) {
  const leftParts = parseVersion(left);
  const rightParts = parseVersion(right);
  if (!leftParts || !rightParts) return null;

  for (let index = 0; index < 3; index += 1) {
    const difference = (leftParts[index] ?? 0) - (rightParts[index] ?? 0);
    if (difference !== 0) return Math.sign(difference);
  }
  return 0;
}

function extractVersion(output) {
  const parsed = parseVersion(output);
  return parsed ? parsed.join('.') : null;
}

function readDeclarations(root) {
  const nodeVersion = readFileSync(resolve(root, '.node-version'), 'utf8').trim();
  const packageJson = JSON.parse(readFileSync(resolve(root, 'package.json'), 'utf8'));
  const rustToolchain = readFileSync(resolve(root, 'rust-toolchain.toml'), 'utf8');
  const rustChannel = rustToolchain.match(/^channel\s*=\s*"([^"]+)"/m)?.[1];

  if (!nodeVersion || !rustChannel || typeof packageJson.packageManager !== 'string') {
    throw new Error('Repository toolchain declarations are incomplete.');
  }

  const pnpmVersion = packageJson.packageManager.match(/^pnpm@(\d+\.\d+\.\d+)$/)?.[1];
  if (!pnpmVersion) {
    throw new Error('packageManager must declare pnpm with an exact version.');
  }

  return { nodeVersion, pnpmVersion, rustChannel };
}

function checkTool({ id, label, command, args, expectedVersion, minimumVersion, validatedVersion, allowNonzeroExit = false, runCommand }) {
  const probe = runCommand(command, args);
  const output = `${probe.stdout}\n${probe.stderr}`;
  const actualVersion = extractVersion(output);

  if (!probe.found) {
    return { id, label, status: 'fail', detail: 'not found' };
  }
  if (probe.exitCode != null && probe.exitCode !== 0 && !allowNonzeroExit) {
    return { id, label, status: 'fail', detail: `command exited ${probe.exitCode}` };
  }
  if (!actualVersion) {
    return { id, label, status: 'fail', detail: 'version could not be read' };
  }
  if (expectedVersion && compareVersions(actualVersion, expectedVersion) !== 0) {
    return { id, label, status: 'fail', detail: `requires ${expectedVersion}; found ${actualVersion}` };
  }
  if (minimumVersion && compareVersions(actualVersion, minimumVersion) < 0) {
    return { id, label, status: 'fail', detail: `requires at least ${minimumVersion}; found ${actualVersion}` };
  }
  if (validatedVersion && compareVersions(actualVersion, validatedVersion) !== 0) {
    return { id, label, status: 'warn', detail: `supported, but validated with ${validatedVersion}; found ${actualVersion}` };
  }

  return { id, label, status: 'pass', detail: actualVersion };
}

function skippedCheck(id, label, platform) {
  return { id, label, status: 'skip', detail: `not evaluated on ${platform}` };
}

export function runDoctor({
  platform = process.platform,
  runCommand = defaultRunCommand,
  repositoryRoot: targetRepositoryRoot = repositoryRoot,
  declarations,
} = {}) {
  const resolvedDeclarations = declarations ?? readDeclarations(targetRepositoryRoot);
  const checks = [
    checkTool({ id: 'node', label: 'Node.js', command: 'node', args: ['--version'], expectedVersion: resolvedDeclarations.nodeVersion, runCommand }),
    checkTool({ id: 'pnpm', label: 'pnpm', command: 'pnpm', args: ['--version'], expectedVersion: resolvedDeclarations.pnpmVersion, runCommand }),
    checkTool({ id: 'rustup', label: 'rustup', command: 'rustup', args: ['--version'], runCommand }),
    checkTool({ id: 'rustc', label: 'rustc', command: 'rustc', args: ['--version'], expectedVersion: resolvedDeclarations.rustChannel, runCommand }),
    checkTool({ id: 'cargo', label: 'Cargo', command: 'cargo', args: ['--version'], expectedVersion: resolvedDeclarations.rustChannel, runCommand }),
    checkTool({ id: 'cmake', label: 'CMake', command: 'cmake', args: ['--version'], minimumVersion: minimumVersions.cmake, validatedVersion: validatedVersions.cmake, runCommand }),
    checkTool({ id: 'ninja', label: 'Ninja', command: 'ninja', args: ['--version'], minimumVersion: minimumVersions.ninja, validatedVersion: validatedVersions.ninja, runCommand }),
    checkTool({ id: 'git', label: 'Git', command: 'git', args: ['--version'], minimumVersion: minimumVersions.git, validatedVersion: validatedVersions.git, runCommand }),
    platform === 'darwin'
      ? checkTool({ id: 'xcode', label: 'Xcode', command: 'xcodebuild', args: ['-version'], minimumVersion: minimumVersions.xcode, validatedVersion: validatedVersions.xcode, runCommand })
      : skippedCheck('xcode', 'Xcode', platform),
    platform === 'win32'
      ? checkTool({ id: 'msvc', label: 'MSVC', command: 'cl', args: [], minimumVersion: minimumVersions.msvc, allowNonzeroExit: true, runCommand })
      : skippedCheck('msvc', 'MSVC', platform),
  ];

  return {
    checks,
    exitCode: checks.some(({ status }) => status === 'fail') ? 1 : 0,
  };
}

function formatReport(report) {
  return report.checks
    .map(({ id, status, detail }) => `${status.toUpperCase().padEnd(4)} ${id}: ${detail}`)
    .join('\n');
}

function main() {
  const report = runDoctor();
  if (process.argv.includes('--json')) {
    process.stdout.write(`${JSON.stringify(report)}\n`);
  } else {
    process.stdout.write(`${formatReport(report)}\n`);
  }
  process.exitCode = report.exitCode;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main();
}
