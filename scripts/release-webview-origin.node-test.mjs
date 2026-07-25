import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import { tauriArgs } from './run-tauri.mjs';
import {
  RELEASE_ORIGINS,
  forbiddenArtifactOrigins,
  releaseConfigurationErrors,
  validateReleaseConfiguration,
} from './validate-release-webview-origin.mjs';

async function repositoryConfiguration() {
  const [base, windows, capability] = await Promise.all(
    [
      'src-tauri/tauri.conf.json',
      'src-tauri/tauri.windows.conf.json',
      'src-tauri/capabilities/default.json',
    ].map(async (file) => JSON.parse(await readFile(file, 'utf8'))),
  );
  return { base, windows, capability };
}

test('release configuration uses only bundled local app origins', async () => {
  await validateReleaseConfiguration();
  assert.deepEqual(RELEASE_ORIGINS, {
    windows: 'https://tauri.localhost',
    linux: 'tauri://localhost',
    macos: 'tauri://localhost',
  });
});

test('release configuration rejects devUrl and remote frontend leakage', async () => {
  const configuration = await repositoryConfiguration();
  configuration.base.build.devUrl = 'http://localhost:1420';
  configuration.base.build.frontendDist = 'https://example.test/app';
  const errors = releaseConfigurationErrors(configuration).join('\n');
  assert.match(errors, /must not contain build\.devUrl/u);
  assert.match(errors, /bundled local directory/u);
});

test('release capability rejects implicit or remote IPC origins', async () => {
  const configuration = await repositoryConfiguration();
  delete configuration.capability.local;
  configuration.capability.remote = { urls: ['http://localhost:1420/*'] };
  const errors = releaseConfigurationErrors(configuration).join('\n');
  assert.match(errors, /explicitly enable only local/u);
  assert.match(errors, /must not grant IPC to remote/u);
});

test('artifact inspection rejects ASCII and UTF-16 development origins', () => {
  assert.deepEqual(forbiddenArtifactOrigins(Buffer.from('ok http://localhost:1420 bad')), [
    'http://localhost:1420',
  ]);
  assert.deepEqual(forbiddenArtifactOrigins(Buffer.from('https://localhost:1420', 'utf16le')), [
    'https://localhost:1420',
  ]);
  assert.deepEqual(forbiddenArtifactOrigins(Buffer.from('https://tauri.localhost')), []);
});

test('Tauri wrapper applies the dev overlay only to development', () => {
  assert.deepEqual(tauriArgs(['dev']), ['dev', '--config', 'src-tauri/tauri.dev.conf.json']);
  assert.deepEqual(tauriArgs(['build', '--no-bundle']), ['build', '--no-bundle']);
  assert.deepEqual(tauriArgs(['dev', '--config', 'custom.json']), [
    'dev',
    '--config',
    'custom.json',
  ]);
});
