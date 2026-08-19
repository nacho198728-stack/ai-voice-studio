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

  return {
    calls,
    runCommand(command, args) {
      calls.push([command, ...args]);
      return responses[[command, ...args].join(' ')] ?? {
        found: false,
        stdout: '',
        stderr: '',
      };
    },
  };
}

test('reports a passing macOS toolchain from compatible command probes', () => {
  const probe = probeFrom({
    'node --version': { found: true, stdout: 'v24.16.0\n', stderr: '' },
    'pnpm --version': { found: true, stdout: '11.19.0\n', stderr: '' },
    'rustup --version': { found: true, stdout: 'rustup 1.29.0\n', stderr: '' },
    'rustc --version': { found: true, stdout: 'rustc 1.97.1 (abc 2026-08-01)\n', stderr: '' },
    'cargo --version': { found: true, stdout: 'cargo 1.97.1 (abc 2026-08-01)\n', stderr: '' },
    'cmake --version': { found: true, stdout: 'cmake version 4.4.2\n', stderr: '' },
    'ninja --version': { found: true, stdout: '1.13.2\n', stderr: '' },
    'git --version': { found: true, stdout: 'git version 2.50.1\n', stderr: '' },
    'xcodebuild -version': { found: true, stdout: 'Xcode 26.6\nBuild version 17A1\n', stderr: '' },
  });

  const report = runDoctor({ platform: 'darwin', runCommand: probe.runCommand });

  assert.equal(report.exitCode, 0);
  assert.deepEqual(
    report.checks.map(({ id, status }) => [id, status]),
    [
      ['node', 'pass'],
      ['pnpm', 'pass'],
      ['rustup', 'pass'],
      ['rustc', 'pass'],
      ['cargo', 'pass'],
      ['cmake', 'pass'],
      ['ninja', 'pass'],
      ['git', 'pass'],
      ['xcode', 'pass'],
      ['msvc', 'skip'],
    ],
  );
  assert.equal(probe.calls.some(([command]) => command === 'cl'), false);
});

test('fails when an exact project pin is incompatible or a required tool is absent', () => {
  const probe = probeFrom({
    'node --version': { found: true, stdout: 'v24.15.0\n', stderr: '' },
    'pnpm --version': { found: false, stdout: '', stderr: '' },
    'rustup --version': { found: true, stdout: 'rustup 1.29.0\n', stderr: '' },
    'rustc --version': { found: true, stdout: 'rustc 1.97.1 (abc 2026-08-01)\n', stderr: '' },
    'cargo --version': { found: true, stdout: 'cargo 1.97.1 (abc 2026-08-01)\n', stderr: '', exitCode: 1 },
    'cmake --version': { found: true, stdout: 'cmake version 4.4.2\n', stderr: '' },
    'ninja --version': { found: true, stdout: '1.13.2\n', stderr: '' },
    'git --version': { found: true, stdout: 'git version 2.50.1\n', stderr: '' },
    'xcodebuild -version': { found: true, stdout: 'Xcode 26.6\n', stderr: '' },
  });

  const report = runDoctor({ platform: 'darwin', runCommand: probe.runCommand });

  assert.equal(report.exitCode, 1);
  assert.deepEqual(
    report.checks
      .filter(({ status }) => status === 'fail')
      .map(({ id }) => id),
    ['node', 'pnpm', 'cargo'],
  );
});

test('evaluates only the Windows compiler prerequisite on Windows', () => {
  const probe = probeFrom({
    'node --version': { found: true, stdout: 'v24.16.0\n', stderr: '' },
    'pnpm --version': { found: true, stdout: '11.19.0\n', stderr: '' },
    'rustup --version': { found: true, stdout: 'rustup 1.29.0\n', stderr: '' },
    'rustc --version': { found: true, stdout: 'rustc 1.97.1 (abc 2026-08-01)\n', stderr: '' },
    'cargo --version': { found: true, stdout: 'cargo 1.97.1 (abc 2026-08-01)\n', stderr: '' },
    'cmake --version': { found: true, stdout: 'cmake version 4.4.2\n', stderr: '' },
    'ninja --version': { found: true, stdout: '1.13.2\n', stderr: '' },
    'git --version': { found: true, stdout: 'git version 2.50.1\n', stderr: '' },
    'cl': { found: true, stdout: '', stderr: 'Microsoft (R) C/C++ Optimizing Compiler Version 19.44.35211 for x64\n' },
  });

  const report = runDoctor({ platform: 'win32', runCommand: probe.runCommand });

  assert.equal(report.exitCode, 0);
  assert.deepEqual(
    report.checks.slice(-2).map(({ id, status }) => [id, status]), [
    ['xcode', 'skip'],
    ['msvc', 'pass'],
  ]);
  assert.equal(probe.calls.some(([command]) => command === 'xcodebuild'), false);
});

test('uses the target repository declarations as the exact version authority', (t) => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), 'aivs-doctor-'));
  t.after(() => rmSync(fixtureRoot, { recursive: true, force: true }));
  writeFileSync(join(fixtureRoot, '.node-version'), '23.11.0\n');
  writeFileSync(join(fixtureRoot, 'package.json'), JSON.stringify({ packageManager: 'pnpm@10.8.1' }));
  writeFileSync(join(fixtureRoot, 'rust-toolchain.toml'), '[toolchain]\nchannel = "1.96.0"\n');

  const probe = probeFrom({
    'node --version': { found: true, stdout: 'v23.11.0\n', stderr: '' },
    'pnpm --version': { found: true, stdout: '10.8.1\n', stderr: '' },
    'rustup --version': { found: true, stdout: 'rustup 1.29.0\n', stderr: '' },
    'rustc --version': { found: true, stdout: 'rustc 1.96.0 (abc 2026-08-01)\n', stderr: '' },
    'cargo --version': { found: true, stdout: 'cargo 1.96.0 (abc 2026-08-01)\n', stderr: '' },
    'cmake --version': { found: true, stdout: 'cmake version 4.4.2\n', stderr: '' },
    'ninja --version': { found: true, stdout: '1.13.2\n', stderr: '' },
    'git --version': { found: true, stdout: 'git version 2.50.1\n', stderr: '' },
    'xcodebuild -version': { found: true, stdout: 'Xcode 26.6\n', stderr: '' },
  });

  const report = runDoctor({
    platform: 'darwin',
    repositoryRoot: fixtureRoot,
    runCommand: probe.runCommand,
  });

  assert.equal(report.exitCode, 0);
});

test('runs the CLI and emits a machine-readable report', () => {
  const result = spawnSync(process.execPath, [fileURLToPath(new URL('./doctor.mjs', import.meta.url)), '--json'], {
    encoding: 'utf8',
  });

  assert.notEqual(result.stdout.trim(), '');
  assert.equal(typeof JSON.parse(result.stdout).exitCode, 'number');
});
