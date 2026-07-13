import { readFile, writeFile } from 'node:fs/promises';

const [inputPath, version, windowsSignaturePath, outputPath] = process.argv.slice(2);

if (!inputPath || !version || !windowsSignaturePath || !outputPath) {
  throw new Error(
    'Usage: node scripts/finalize-updater-manifest.mjs <manifest> <version> <windows-signature> <output>',
  );
}

const manifest = JSON.parse(await readFile(inputPath, 'utf8'));
const windowsSignature = (await readFile(windowsSignaturePath, 'utf8')).trim();
const releaseBase = `https://github.com/Bakobiibizo/harbor/releases/download/v${version}`;
const windowsUrl = `${releaseBase}/Harbor_${version}_x64-setup.exe`;

for (const platform of Object.values(manifest.platforms ?? {})) {
  if (typeof platform.url === 'string') {
    platform.url = platform.url.replace(
      'https://github.com/Bakobiibizo/harbor/releases/latest/download',
      releaseBase,
    );
  }
}

manifest.platforms['windows-x86_64'] = {
  signature: windowsSignature,
  url: windowsUrl,
};
manifest.platforms['windows-x86_64-nsis'] = {
  signature: windowsSignature,
  url: windowsUrl,
};

await writeFile(outputPath, `${JSON.stringify(manifest, null, 2)}\n`);
