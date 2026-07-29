import { readFile, readdir, stat } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

export const RELEASE_ORIGINS = Object.freeze({
  windows: 'https://tauri.localhost',
  linux: 'tauri://localhost',
  macos: 'tauri://localhost',
});

export const FORBIDDEN_ARTIFACT_ORIGINS = Object.freeze([
  'http://localhost:1420',
  'https://localhost:1420',
  'http://127.0.0.1:1420',
  'https://127.0.0.1:1420',
]);

function own(object, key) {
  return Object.prototype.hasOwnProperty.call(object ?? {}, key);
}

function isRemoteUrl(value) {
  return typeof value === 'string' && /^[a-z][a-z\d+.-]*:\/\//iu.test(value);
}

export function releaseConfigurationErrors({ base, windows, capability }) {
  const errors = [];
  const build = base?.build ?? {};

  if (own(build, 'devUrl')) {
    errors.push('src-tauri/tauri.conf.json must not contain build.devUrl');
  }
  if (own(build, 'beforeDevCommand')) {
    errors.push('src-tauri/tauri.conf.json must not contain build.beforeDevCommand');
  }
  if (typeof build.frontendDist !== 'string' || isRemoteUrl(build.frontendDist)) {
    errors.push('release build.frontendDist must be a bundled local directory');
  }

  for (const window of base?.app?.windows ?? []) {
    if (own(window, 'url')) {
      errors.push("release windows must use Tauri's bundled App URL, not an explicit URL");
    }
  }

  const windowsWindows = windows?.app?.windows ?? [];
  if (
    windowsWindows.length === 0 ||
    windowsWindows.some((window) => window.useHttpsScheme !== true)
  ) {
    errors.push('every Windows release webview must set useHttpsScheme to true');
  }
  if (windowsWindows.some((window) => own(window, 'url'))) {
    errors.push("Windows release windows must use Tauri's bundled App URL");
  }

  if (capability?.local !== true) {
    errors.push('the main-window capability must explicitly enable only local app URLs');
  }
  if (own(capability, 'remote')) {
    errors.push('the main-window capability must not grant IPC to remote URLs');
  }
  if (!(capability?.windows ?? []).includes('main')) {
    errors.push('the local capability must cover the main window');
  }

  return errors;
}

export async function validateReleaseConfiguration(projectRoot = root) {
  const [base, windows, capability] = await Promise.all(
    [
      'src-tauri/tauri.conf.json',
      'src-tauri/tauri.windows.conf.json',
      'src-tauri/capabilities/default.json',
    ].map(async (relativePath) =>
      JSON.parse(await readFile(path.join(projectRoot, relativePath), 'utf8')),
    ),
  );
  const errors = releaseConfigurationErrors({ base, windows, capability });
  if (errors.length > 0) {
    throw new Error(errors.join('\n'));
  }
}

function utf16Bytes(value, littleEndian) {
  const bytes = Buffer.alloc(value.length * 2);
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    bytes[index * 2 + (littleEndian ? 0 : 1)] = code & 0xff;
    bytes[index * 2 + (littleEndian ? 1 : 0)] = code >> 8;
  }
  return bytes;
}

export function forbiddenArtifactOrigins(bytes) {
  const buffer = Buffer.isBuffer(bytes) ? bytes : Buffer.from(bytes);
  return FORBIDDEN_ARTIFACT_ORIGINS.filter((origin) =>
    [Buffer.from(origin), utf16Bytes(origin, true), utf16Bytes(origin, false)].some(
      (needle) => buffer.indexOf(needle) >= 0,
    ),
  );
}

async function artifactFiles(input) {
  const metadata = await stat(input);
  if (metadata.isFile()) return [input];
  if (!metadata.isDirectory()) return [];

  const entries = await readdir(input, { withFileTypes: true });
  const nested = await Promise.all(
    entries
      .filter((entry) => !entry.isSymbolicLink())
      .map((entry) => artifactFiles(path.join(input, entry.name))),
  );
  return nested.flat();
}

export async function validateArtifacts(inputs) {
  for (const input of inputs) {
    for (const file of await artifactFiles(path.resolve(input))) {
      const leaked = forbiddenArtifactOrigins(await readFile(file));
      if (leaked.length > 0) {
        throw new Error(`${file} contains development webview origin(s): ${leaked.join(', ')}`);
      }
    }
  }
}

async function main(args) {
  const artifacts = [];
  for (let index = 0; index < args.length; index += 1) {
    if (args[index] !== '--artifact' || !args[index + 1]) {
      throw new Error(`unknown or incomplete argument: ${args[index]}`);
    }
    artifacts.push(args[index + 1]);
    index += 1;
  }

  await validateReleaseConfiguration();
  await validateArtifacts(artifacts);
  console.log(
    `Release webview origin integrity passed (${Object.entries(RELEASE_ORIGINS)
      .map(([platform, origin]) => `${platform}=${origin}`)
      .join(', ')}).`,
  );
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    await main(process.argv.slice(2));
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  }
}
