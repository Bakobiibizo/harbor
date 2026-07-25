import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { chmod, copyFile, mkdtemp, mkdir, readFile, symlink, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { promisify } from 'node:util';
import test from 'node:test';

const execFileAsync = promisify(execFile);
const repoRoot = path.resolve(import.meta.dirname, '..');
const packageScript = path.join(repoRoot, 'scripts/package-linux-webrtc-runtime.sh');

async function fixture() {
  const root = await mkdtemp(path.join(os.tmpdir(), 'harbor-webrtc-package-'));
  const runtime = path.join(root, 'runtime');
  const lib = path.join(runtime, 'lib');
  const injected = path.join(lib, 'webkit2gtk-4.1/injected-bundle');
  const libexec = path.join(runtime, 'libexec/webkit2gtk-4.1');
  const manifestDir = path.join(runtime, 'share/harbor');
  await Promise.all([
    mkdir(injected, { recursive: true }),
    mkdir(libexec, { recursive: true }),
    mkdir(manifestDir, { recursive: true }),
  ]);
  await copyFile('/bin/true', path.join(lib, 'libwebkit2gtk-4.1.so.0.21.7'));
  await copyFile('/bin/true', path.join(lib, 'libjavascriptcoregtk-4.1.so.0.10.11'));
  await symlink('libwebkit2gtk-4.1.so.0.21.7', path.join(lib, 'libwebkit2gtk-4.1.so.0'));
  await symlink(
    'libjavascriptcoregtk-4.1.so.0.10.11',
    path.join(lib, 'libjavascriptcoregtk-4.1.so.0'),
  );
  await copyFile('/bin/true', path.join(injected, 'libwebkit2gtkinjectedbundle.so'));
  for (const processName of ['WebKitWebProcess', 'WebKitNetworkProcess', 'WebKitGPUProcess']) {
    const processPath = path.join(libexec, processName);
    await copyFile('/bin/true', processPath);
    await chmod(processPath, 0o755);
  }
  await writeFile(
    path.join(manifestDir, 'webrtc-runtime.env'),
    'HARBOR_WEBKIT_RUNTIME_FORMAT=1\nENABLE_WEB_RTC=ON\nUSE_GSTREAMER_WEBRTC=ON\n',
  );
  return { root, runtime };
}

test('packages a private WebKitGTK runtime and launches Harbor through it', async () => {
  const { root, runtime } = await fixture();
  const output = path.join(root, 'harbor-linux.tar.gz');
  const packageArguments = ['--runtime', runtime, '--harbor', '/bin/true', '--output', output];
  const packageEnvironment = { ...process.env, SOURCE_DATE_EPOCH: '1234567890' };
  await execFileAsync(packageScript, packageArguments, { env: packageEnvironment });

  const secondOutput = path.join(root, 'harbor-linux-second.tar.gz');
  const secondArguments = [...packageArguments];
  secondArguments[secondArguments.length - 1] = secondOutput;
  await execFileAsync(packageScript, secondArguments, { env: packageEnvironment });
  assert.deepEqual(await readFile(output), await readFile(secondOutput));

  const extract = path.join(root, 'extract');
  await mkdir(extract);
  await execFileAsync('tar', ['-xzf', output, '-C', extract]);
  const packageRoot = path.join(extract, 'harbor');
  const probe = path.join(packageRoot, 'libexec/harbor-bin');
  await writeFile(
    probe,
    '#!/bin/sh\nprintf "%s\\n" "$HARBOR_WEBKIT_RUNTIME_DIR" "$WEBKIT_EXEC_PATH" "$WEBKIT_INJECTED_BUNDLE_PATH" "$LD_LIBRARY_PATH" "$1"\n',
  );
  await chmod(probe, 0o755);

  const { stdout } = await execFileAsync(path.join(packageRoot, 'bin/harbor'), ['argument']);
  const lines = stdout.trim().split('\n');
  assert.equal(lines[0], path.join(packageRoot, 'runtime'));
  assert.equal(lines[1], path.join(packageRoot, 'runtime/libexec/webkit2gtk-4.1'));
  assert.equal(lines[2], path.join(packageRoot, 'runtime/lib/webkit2gtk-4.1/injected-bundle'));
  assert.match(lines[3], new RegExp(`^${path.join(packageRoot, 'runtime/lib')}`));
  assert.equal(lines[4], 'argument');
  assert.match(
    await readFile(path.join(packageRoot, 'MANIFEST'), 'utf8'),
    /HARBOR_EXECUTABLE_SHA256=/u,
  );
});

test('refuses a runtime that does not attest WebRTC support', async () => {
  const { root, runtime } = await fixture();
  await writeFile(path.join(runtime, 'share/harbor/webrtc-runtime.env'), 'ENABLE_WEB_RTC=OFF\n');
  await assert.rejects(
    execFileAsync(packageScript, [
      '--runtime',
      runtime,
      '--harbor',
      '/bin/true',
      '--output',
      path.join(root, 'bad.tar.gz'),
    ]),
    /Runtime manifest does not attest WebRTC support/u,
  );
});

test('refuses a mixed-architecture or invalid runtime process', async () => {
  const { root, runtime } = await fixture();
  await writeFile(
    path.join(runtime, 'libexec/webkit2gtk-4.1/WebKitNetworkProcess'),
    '#!/bin/sh\nexit 0\n',
  );
  await assert.rejects(
    execFileAsync(packageScript, [
      '--runtime',
      runtime,
      '--harbor',
      '/bin/true',
      '--output',
      path.join(root, 'mixed-architecture.tar.gz'),
    ]),
    /Unsupported or invalid ELF machine/u,
  );
});
