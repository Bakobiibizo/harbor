import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const devConfig = 'src-tauri/tauri.dev.conf.json';

export function tauriArgs(input) {
  const args = [...input];
  const commandIndex = args.findIndex((argument) => !argument.startsWith('-'));

  if (
    commandIndex >= 0 &&
    args[commandIndex] === 'dev' &&
    !args.includes('--config') &&
    !args.includes('-c')
  ) {
    args.splice(commandIndex + 1, 0, '--config', devConfig);
  }

  return args;
}

export async function runTauri(input = process.argv.slice(2)) {
  const cli = path.join(root, 'node_modules', '@tauri-apps', 'cli', 'tauri.js');
  const child = spawn(process.execPath, [cli, ...tauriArgs(input)], {
    cwd: root,
    env: process.env,
    stdio: 'inherit',
  });

  for (const signal of ['SIGINT', 'SIGTERM']) {
    process.once(signal, () => child.kill(signal));
  }

  return await new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('exit', (code, signal) => {
      if (signal) {
        reject(new Error(`Tauri CLI terminated by ${signal}`));
      } else {
        resolve(code ?? 1);
      }
    });
  });
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    process.exitCode = await runTauri();
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  }
}
