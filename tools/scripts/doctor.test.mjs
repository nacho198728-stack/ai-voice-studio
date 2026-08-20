import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import { runDoctor } from './doctor.mjs';

function probeFrom(responses) {
  const calls = [];
  const invocations = [];

  return {
    calls,
    invocations,
    runCommand(command, args, options) {
      calls.push([command, ...args]);
      invocations.push({ command, args, options });
      return responses[[command, ...args].join(' ')] ?? {
        found: false,
        stdout: '',
        stderr: '',
        exitCode: null,
      };
    },
  };
}

function passingResponses({ nodeVersion = '24.16.0', pnpmVersion = '11.19.0', rustVersion = '1.97.1' } = {}) {
  const toolchainPath = `/toolchains/${rustVersion}`;

  return {
    'node --version': { found: true, stdout: `v${nodeVersion}\n`, stderr: '', exitCode: 0 },
    'pnpm --version': { found: true, stdout: `${pnpmVersion}\n`, stderr: '', exitCode: 0 },
    'pnpm.cmd --version': { found: true, stdout: `${pnpmVersion}\n`, stderr: '', exitCode: 0 },
    'rustup --version': { found: true, stdout: 'rustup 1.29.0\n', stderr: '', exitCode: 0 },
    [`rustup which --toolchain ${rustVersion} rustc`]: { found: true, stdout: `${toolchainPath}/rustc\n`, stderr: '', exitCode: 0 },
    [`rustup which --toolchain ${rustVersion} cargo`]: { found: true, stdout: `${toolchainPath}/cargo\n`, stderr: '', exitCode: 0 },
    [`${toolchainPath}/rustc --version`]: { found: true, stdout: `rustc ${rustVersion} (abc 2026-08-01)\n`, stderr: '', exitCode: 0 },
    [`${toolchainPath}/cargo --version`]: { found: true, stdout: `cargo ${rustVersion} (abc 2026-08-01)\n`, stderr: '', exitCode: 0 },
    'cmake --version': { found: true, stdout: 'cmake version 4.4.2\n', stderr: '', exitCode: 0 },
    'ninja --version': { found: true, stdout: '1.13.2\n', stderr: '', exitCode: 0 },
    'git --version': { found: true, stdout: 'git version 2.50.1\n', stderr: '', exitCode: 0 },
    'xcodebuild -version': { found: true, stdout: 'Xcode 26.6\nBuild version 17A1\n', stderr: '', exitCode: 0 },
  };
}

test('reports a passing macOS toolchain from compatible command probes', () => {
  const probe = probeFrom(passingResponses());
  const report = runDoctor({ platform: 'darwin', runCommand: probe.runCommand });

  assert.equal(report.exitCode, 0);
  assert.deepEqual(
    report.checks.map(({ id, status }) => [id, status]),
    [
      ['node', 'pass'], ['pnpm', 'pass'], ['rustup', 'pass'], ['rustc', 'pass'], ['cargo', 'pass'],
      ['cmake', 'pass'], ['ninja', 'pass'], ['git', 'pass'], ['xcode', 'pass'], ['msvc', 'skip'],
    ],
  );
  assert.equal(probe.calls.some(([command]) => command === 'cl'), false);
});

test('fails when an exact project pin is incompatible or a required tool is absent', () => {
  const probe = probeFrom({
    ...passingResponses(),
    'node --version': { found: true, stdout: 'v24.15.0\n', stderr: '', exitCode: 0 },
    'pnpm --version': { found: false, stdout: '', stderr: '', exitCode: null },
    '/toolchains/1.97.1/cargo --version': { found: true, stdout: 'cargo 1.97.1 (abc 2026-08-01)\n', stderr: '', exitCode: 1 },
  });
  const report = runDoctor({ platform: 'darwin', runCommand: probe.runCommand });

  assert.equal(report.exitCode, 1);
  assert.deepEqual(report.checks.filter(({ status }) => status === 'fail').map(({ id }) => id), ['node', 'pnpm', 'cargo']);
});

test('uses the documented MSVC usage outcome but rejects unrelated nonzero exits', () => {
  const normalProbe = probeFrom({
    ...passingResponses(),
    cl: {
      found: true,
      stdout: '',
      stderr: 'Microsoft (R) C/C++ Optimizing Compiler Version 19.44.35211 for x64\nCopyright (C) Microsoft Corporation.\nusage: cl [ option... ] filename... [ /link linkoption... ]\n',
      exitCode: 2,
    },
  });
  const normalReport = runDoctor({ platform: 'win32', runCommand: normalProbe.runCommand });
  assert.equal(normalReport.exitCode, 0);
  assert.equal(normalReport.checks.at(-1).status, 'pass');
  assert.equal(
    normalProbe.invocations.find(({ command }) => command.startsWith('pnpm'))?.command,
    'pnpm.cmd',
    'Windows must probe the Corepack command shim that spawnSync can execute without a shell',
  );

  const failedProbe = probeFrom({
    ...passingResponses(),
    cl: {
      found: true,
      stdout: '',
      stderr: 'Microsoft (R) C/C++ Optimizing Compiler Version 19.44.35211 for x64\nfatal error C1083: Cannot open include file\n',
      exitCode: 2,
    },
  });
  const failedReport = runDoctor({ platform: 'win32', runCommand: failedProbe.runCommand });
  assert.equal(failedReport.exitCode, 1);
  assert.equal(failedReport.checks.at(-1).status, 'fail');
  assert.equal(normalProbe.calls.some(([command]) => command === 'xcodebuild'), false);
});

test('uses the target repository declarations as the exact version authority', (t) => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), 'aivs-doctor-'));
  t.after(() => rmSync(fixtureRoot, { recursive: true, force: true }));
  writeFileSync(join(fixtureRoot, '.node-version'), '23.11.0\n');
  writeFileSync(join(fixtureRoot, 'package.json'), JSON.stringify({ packageManager: 'pnpm@10.8.1' }));
  writeFileSync(join(fixtureRoot, 'rust-toolchain.toml'), '[toolchain]\nchannel = "1.96.0"\n');

  const probe = probeFrom(passingResponses({ nodeVersion: '23.11.0', pnpmVersion: '10.8.1', rustVersion: '1.96.0' }));
  const report = runDoctor({ platform: 'darwin', repositoryRoot: fixtureRoot, runCommand: probe.runCommand });
  assert.equal(report.exitCode, 0);
});

test('uses non-installing Corepack and rustup resolution before executable probes', () => {
  const probe = probeFrom(passingResponses());
  const report = runDoctor({ platform: 'darwin', runCommand: probe.runCommand });

  assert.equal(report.exitCode, 0);
  assert.deepEqual(probe.invocations.find(({ command }) => command === 'pnpm'), {
    command: 'pnpm',
    args: ['--version'],
    options: { environment: { COREPACK_ENABLE_NETWORK: '0' } },
  });
  assert.deepEqual(probe.invocations.find(({ command, args }) => command === 'rustup' && args[0] === 'which'), {
    command: 'rustup',
    args: ['which', '--toolchain', '1.97.1', 'rustc'],
    options: { environment: { RUSTUP_AUTO_INSTALL: '0' } },
  });
  assert.equal(probe.calls.some(([command]) => command === 'rustc' || command === 'cargo'), false);
  assert.equal(probe.calls.some(([command]) => command === '/toolchains/1.97.1/rustc'), true);
  assert.equal(probe.calls.some(([command]) => command === '/toolchains/1.97.1/cargo'), true);
});

test('fails probes terminated by a signal or without an exit status', () => {
  const probe = probeFrom({
    ...passingResponses(),
    'node --version': { found: true, stdout: 'v24.16.0\n', stderr: '', exitCode: null, signal: 'SIGTERM' },
    'pnpm --version': { found: true, stdout: '11.19.0\n', stderr: '', exitCode: null },
  });
  const report = runDoctor({ platform: 'darwin', runCommand: probe.runCommand });

  assert.equal(report.exitCode, 1);
  assert.deepEqual(report.checks.filter(({ status }) => status === 'fail').map(({ id }) => id), ['node', 'pnpm']);
});

test('runs the CLI and emits a machine-readable report', () => {
  const result = spawnSync(process.execPath, [fileURLToPath(new URL('./doctor.mjs', import.meta.url)), '--json'], {
    encoding: 'utf8',
  });

  assert.notEqual(result.stdout.trim(), '');
  assert.equal(typeof JSON.parse(result.stdout).exitCode, 'number');
});
