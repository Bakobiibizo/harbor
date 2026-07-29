import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';

const script = resolve('scripts/live-beta-acceptance.mjs');

function run(args) {
  return spawnSync(process.execPath, [script, ...args], { encoding: 'utf8' });
}

function withSession(callback) {
  const root = mkdtempSync(join(tmpdir(), 'harbor acceptance '));
  const directory = join(root, 'evidence with spaces');
  const initialized = run(['init', '--dir', directory]);
  assert.equal(initialized.status, 0, initialized.stderr);
  try {
    callback({ root, directory, manifest: join(directory, 'session.json') });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

function writeEvidence(path, text = 'sanitized acceptance observation') {
  writeFileSync(path, `${text}\n`, 'utf8');
}

function evidenceRecord(path, relativePath) {
  const content = readFileSync(path);
  return {
    path: relativePath,
    bytes: content.byteLength,
    sha256: createHash('sha256').update(content).digest('hex'),
  };
}

function makeCompleteManifest(directory, manifestPath) {
  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
  Object.assign(manifest.metadata, {
    commit: 'a'.repeat(40),
    version: '1.4.1-beta.7',
    productionDownloadUrl: 'https://social-harbor.com/download/test',
    windowsVersion: 'Windows 11 24H2 build test',
    windowsArchitecture: 'x86_64',
    windowsPackage: 'Harbor_test_x64.msi',
    windowsPackageSha256: 'b'.repeat(64),
    windowsSignatureStatus: 'valid',
    macosVersion: 'macOS test',
    macosArchitecture: 'arm64',
    macosPackage: 'Harbor_test_universal.dmg',
    macosPackageSha256: 'c'.repeat(64),
    macosCodeSignStatus: 'valid',
    macosGatekeeperStatus: 'accepted',
    macosNotarizationStatus: 'accepted',
    thirdProfilePlatform: 'macOS',
    thirdProfileArchitecture: 'arm64',
    relayArtifact: 'relay-test',
    relayArtifactSha256: 'd'.repeat(64),
    relayNamespace: 'harbor.social',
    operator: 'QA',
    evidenceRedactionAttestation: 'confirmed',
  });
  for (const [id, scenario] of Object.entries(manifest.scenarios)) {
    scenario.outcome = 'pass';
    scenario.note = `${id} passed every required packaged observation`;
    scenario.recordedAt = new Date().toISOString();
    scenario.evidence = [];
    for (let index = 0; index < scenario.minEvidence; index += 1) {
      const name = `${id.toLowerCase()}-${index + 1}.txt`;
      const path = join(directory, name);
      writeEvidence(path, `${id} packaged observation ${index + 1}`);
      scenario.evidence.push(evidenceRecord(path, name));
    }
  }
  writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, 'utf8');
  return manifest;
}

test('--dir parsing works with spaces and extra or unknown arguments fail closed', () => {
  withSession(({ directory }) => {
    assert.equal(run(['check', '--dir', directory, 'ignored']).status, 2);
    assert.equal(run(['check', '--unknown', 'value', '--dir', directory]).status, 2);
    assert.equal(run(['init', 'another-directory', '--dir', directory]).status, 2);
    assert.equal(
      run(['check', '--dir', 'artifacts/live-beta-acceptance', '--dir', directory]).status,
      2,
    );
  });
});

test('acceptance runbook links resolve to repository files', () => {
  for (const document of [
    resolve('docs/live-beta-cross-platform-acceptance.md'),
    resolve('docs/windows-macos-call-acceptance.md'),
  ]) {
    const markdown = readFileSync(document, 'utf8');
    for (const match of markdown.matchAll(/\]\(([^)#]+\.md)(?:#[^)]+)?\)/g)) {
      assert.equal(
        existsSync(resolve(dirname(document), match[1])),
        true,
        `${document} has a broken link to ${match[1]}`,
      );
    }
  }
});

test('check rejects missing scenarios and metadata instead of iterating only supplied keys', () => {
  withSession(({ directory, manifest }) => {
    const data = JSON.parse(readFileSync(manifest, 'utf8'));
    data.metadata = {};
    data.scenarios = {};
    writeFileSync(manifest, JSON.stringify(data), 'utf8');

    const result = run(['check', '--dir', directory]);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /metadata\.commit is missing or invalid/);
    assert.match(result.stderr, /R01 is missing/);
    assert.doesNotMatch(result.stdout, /gate PASS/);
  });
});

test('record accepts only enough non-empty evidence files contained by the session', () => {
  withSession(({ root, directory, manifest }) => {
    const insideOne = join(directory, 'inside one.txt');
    const insideTwo = join(directory, 'inside two.txt');
    const outside = join(root, 'outside.txt');
    const empty = join(directory, 'empty.txt');
    writeEvidence(insideOne, 'sanitized Windows packaged observation');
    writeEvidence(insideTwo, 'sanitized macOS packaged observation');
    writeEvidence(outside);
    writeFileSync(empty, '', 'utf8');

    const oneFile = run([
      'record',
      'R01',
      'pass',
      'Both packaged identities resumed',
      insideOne,
      '--dir',
      directory,
    ]);
    assert.equal(oneFile.status, 2);
    assert.match(oneFile.stderr, /requires at least 2 evidence files/);

    assert.equal(
      run(['metadata', 'evidenceRedactionAttestation', 'confirmed', '--dir', directory]).status,
      0,
    );
    assert.equal(
      run([
        'record',
        'R01',
        'pass',
        'Both packaged identities resumed',
        outside,
        insideTwo,
        '--dir',
        directory,
      ]).status,
      2,
    );
    assert.equal(
      run([
        'record',
        'R01',
        'pass',
        'Both packaged identities resumed',
        empty,
        insideTwo,
        '--dir',
        directory,
      ]).status,
      2,
    );
    assert.equal(
      run([
        'record',
        'R01',
        'pass',
        'Both packaged identities resumed',
        manifest,
        insideTwo,
        '--dir',
        directory,
      ]).status,
      2,
    );
    assert.equal(
      run([
        'record',
        'R01',
        'pass',
        'Both packaged identities resumed',
        insideOne,
        insideTwo,
        '--dir',
        directory,
      ]).status,
      0,
    );
    assert.equal(
      JSON.parse(readFileSync(manifest, 'utf8')).metadata.evidenceRedactionAttestation,
      '',
    );
    assert.equal(
      run([
        'record',
        'R01',
        'pass',
        'Both packaged identities resumed',
        insideOne,
        insideOne,
        '--dir',
        directory,
      ]).status,
      2,
    );
  });
});

test('text evidence with high-risk signaling or secrets is rejected', () => {
  withSession(({ directory }) => {
    const safe = join(directory, 'safe.txt');
    const unsafe = join(directory, 'unsafe.log');
    writeEvidence(safe);
    writeEvidence(unsafe, 'password=actual-secret-value');

    const result = run([
      'record',
      'R01',
      'pass',
      'Both packaged identities resumed',
      safe,
      unsafe,
      '--dir',
      directory,
    ]);
    assert.equal(result.status, 2);
    assert.match(result.stderr, /appears to contain secret field/);
  });
});

test('a complete exact manifest passes, while evidence reuse or scenario removal blocks', () => {
  withSession(({ directory, manifest }) => {
    let complete = makeCompleteManifest(directory, manifest);
    const passing = run(['check', '--dir', directory]);
    assert.equal(passing.status, 0, passing.stderr);
    assert.match(passing.stdout, /Live beta gate PASS/);

    writeEvidence(
      join(directory, complete.scenarios.R01.evidence[0].path),
      'evidence changed after redaction review',
    );
    const changed = run(['check', '--dir', directory]);
    assert.equal(changed.status, 1);
    assert.match(changed.stderr, /R01 evidence changed after recording/);

    complete = makeCompleteManifest(directory, manifest);

    complete.scenarios.R02.evidence[0] = complete.scenarios.R01.evidence[0];
    writeFileSync(manifest, JSON.stringify(complete), 'utf8');
    const reused = run(['check', '--dir', directory]);
    assert.equal(reused.status, 1);
    assert.match(reused.stderr, /reuses evidence already assigned to R01/);

    delete complete.scenarios.R15;
    writeFileSync(manifest, JSON.stringify(complete), 'utf8');
    const missing = run(['check', '--dir', directory]);
    assert.equal(missing.status, 1);
    assert.match(missing.stderr, /R15 is missing/);
  });
});

test('invalid package identity and signature metadata cannot pass', () => {
  withSession(({ directory, manifest }) => {
    const complete = makeCompleteManifest(directory, manifest);
    complete.metadata.windowsPackageSha256 = 'not-a-hash';
    complete.metadata.macosNotarizationStatus = 'unknown';
    writeFileSync(manifest, JSON.stringify(complete), 'utf8');

    const result = run(['check', '--dir', directory]);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /metadata\.windowsPackageSha256 is missing or invalid/);
    assert.match(result.stderr, /metadata\.macosNotarizationStatus is missing or invalid/);
  });
});
