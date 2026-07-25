#!/usr/bin/env node

import { randomUUID } from 'node:crypto';
import { spawn } from 'node:child_process';
import { chmodSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, isAbsolute, resolve } from 'node:path';
import process from 'node:process';
import { pathToFileURL } from 'node:url';

const ENDPOINT_KINDS = new Set(['local-wsl', 'windows-powershell', 'remote-linux-ssh']);
const RUNTIME_ENVIRONMENT_KEYS = new Set([
  'DBUS_SESSION_BUS_ADDRESS',
  'DISPLAY',
  'GDK_BACKEND',
  'LD_LIBRARY_PATH',
  'PIPEWIRE_REMOTE',
  'PULSE_SERVER',
  'WAYLAND_DISPLAY',
  'WEBKIT_EXEC_PATH',
  'WEBKIT_INJECTED_BUNDLE_PATH',
  'XDG_RUNTIME_DIR',
]);
const TERMINAL_CALL_STATES = new Set(['idle', 'ended', 'failed']);
const SAFE_CALL_STATES = new Set([
  'idle',
  'requesting_microphone',
  'ringing',
  'incoming',
  'connecting',
  'connected',
  'ended',
  'failed',
]);
const SAFE_CALL_TERMINAL_REASONS = new Set([
  'normal',
  'busy',
  'declined',
  'error',
  'timeout',
  'permission_denied',
  'missing_media_api',
  'missing_device',
  'peer_disconnected',
  'ice_failed',
  'remote_hangup',
]);
const SAFE_ICE_STATES = new Set([
  'new',
  'checking',
  'connected',
  'completed',
  'disconnected',
  'failed',
  'closed',
]);
const SAFE_GROUP_STATES = new Set([
  'idle',
  'starting',
  'ringing',
  'connecting',
  'connected',
  'degraded',
  'ended',
  'failed',
]);
const SAFE_GROUP_PARTICIPANT_STATES = new Set([
  'invited',
  'ringing',
  'connecting',
  'connected',
  'degraded',
  'declined',
  'timed_out',
  'disconnected',
  'left',
  'failed',
]);
const DEFAULTS = Object.freeze({
  commandTimeoutMs: 40_000,
  convergenceTimeoutMs: 60_000,
  callTimeoutConvergenceMs: 65_000,
  pollIntervalMs: 500,
  cleanupTimeoutMs: 15_000,
});

export class HarnessError extends Error {
  constructor(message, code = 'harness_failed') {
    super(message);
    this.name = 'HarnessError';
    this.code = code;
  }
}

function requiredString(value, field) {
  if (typeof value !== 'string' || value.trim() === '') {
    throw new HarnessError(`${field} is required`, 'invalid_config');
  }
  return value;
}

function duration(value, fallback, field) {
  const selected = value ?? fallback;
  if (!Number.isInteger(selected) || selected < 25 || selected > 600_000) {
    throw new HarnessError(`${field} must be an integer between 25 and 600000`, 'invalid_config');
  }
  return selected;
}

function safeEphemeralSecretFile(value, field, kind) {
  const path = requiredString(value, field);
  const absolute =
    isAbsolute(path) || (kind === 'windows-powershell' && /^[A-Za-z]:[\\/]/.test(path));
  const basename = path.split(/[\\/]/).at(-1) ?? '';
  if (!absolute || !basename.startsWith('harbor-call-harness-')) {
    throw new HarnessError(
      `${field} must be an absolute file whose name starts with harbor-call-harness-`,
      'invalid_config',
    );
  }
  return path;
}

function secretSource(value, field, remoteFileRequired = false, kind = 'local-wsl') {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new HarnessError(`${field} must select an env or file source`, 'invalid_config');
  }
  if ('value' in value || 'token' in value || 'password' in value || 'passphrase' in value) {
    throw new HarnessError(`${field} cannot contain inline secret material`, 'invalid_config');
  }
  const keys = ['env', 'file'].filter((key) => value[key] !== undefined);
  if (keys.length !== 1) {
    throw new HarnessError(`${field} must select exactly one of env or file`, 'invalid_config');
  }
  const allowedKeys = new Set(keys[0] === 'file' ? ['file', 'ownership'] : ['env']);
  const unknownKey = Object.keys(value).find((key) => !allowedKeys.has(key));
  if (unknownKey) {
    throw new HarnessError(`${field}.${unknownKey} is not supported`, 'invalid_config');
  }
  if (remoteFileRequired && keys[0] !== 'file') {
    throw new HarnessError(`${field} must use a host-local token file for SSH`, 'invalid_config');
  }
  const source = requiredString(value[keys[0]], `${field}.${keys[0]}`);
  if (keys[0] === 'env' && !/^[A-Z_][A-Z0-9_]*$/.test(source)) {
    throw new HarnessError(`${field}.env is not a valid environment variable`, 'invalid_config');
  }
  if (keys[0] === 'env') return { env: source, ownership: 'operator' };
  const ownership = value.ownership ?? 'operator';
  if (!['operator', 'harness-ephemeral'].includes(ownership)) {
    throw new HarnessError(
      `${field}.ownership must be operator or harness-ephemeral`,
      'invalid_config',
    );
  }
  return {
    file:
      ownership === 'harness-ephemeral'
        ? safeEphemeralSecretFile(source, `${field}.file`, kind)
        : source,
    ownership,
  };
}

function assertPackagedPath(value, field) {
  const path = requiredString(value, field);
  if (
    /(?:^|[\\/])target[\\/](?:debug|release)(?:[\\/]|$)|\bcargo\b|\bpnpm\b|tauri\s+dev/i.test(path)
  ) {
    throw new HarnessError(
      `${field} must name an installed or packaged artifact, not a source-build command/path`,
      'invalid_config',
    );
  }
  return path;
}

function safeDataDirectory(value, field, kind) {
  const path = requiredString(value, field);
  const absolute =
    isAbsolute(path) || (kind === 'windows-powershell' && /^[A-Za-z]:[\\/]/.test(path));
  if (!absolute || !path.includes('harbor-call-harness-')) {
    throw new HarnessError(
      `${field} must be an absolute disposable path containing harbor-call-harness-`,
      'invalid_config',
    );
  }
  return path;
}

function runtimeEnvironment(value, field) {
  if (value === undefined) return {};
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new HarnessError(`${field} must be an object`, 'invalid_config');
  }
  return Object.fromEntries(
    Object.entries(value).map(([name, rawValue]) => {
      if (!RUNTIME_ENVIRONMENT_KEYS.has(name)) {
        throw new HarnessError(
          `${field}.${name} is not an allowed runtime variable`,
          'invalid_config',
        );
      }
      return [name, requiredString(rawValue, `${field}.${name}`)];
    }),
  );
}

export function validateHarnessConfig(input) {
  if (!input || typeof input !== 'object' || input.schemaVersion !== 1) {
    throw new HarnessError('schemaVersion must be 1', 'invalid_config');
  }
  if (!Array.isArray(input.endpoints) || input.endpoints.length < 3) {
    throw new HarnessError('at least three endpoints are required', 'invalid_config');
  }
  const ids = new Set();
  const profiles = new Set();
  const ports = new Set();
  const endpoints = input.endpoints.map((raw, index) => {
    const prefix = `endpoints[${index}]`;
    const id = requiredString(raw?.id, `${prefix}.id`);
    if (!/^[a-z][a-z0-9-]{0,31}$/.test(id) || ids.has(id)) {
      throw new HarnessError(`${prefix}.id must be a unique safe slug`, 'invalid_config');
    }
    ids.add(id);
    const kind = requiredString(raw.kind, `${prefix}.kind`);
    if (!ENDPOINT_KINDS.has(kind)) {
      throw new HarnessError(`${prefix}.kind is unsupported`, 'invalid_config');
    }
    const profile = requiredString(raw.profile, `${prefix}.profile`);
    if (!/^harness-[a-z0-9-]+$/.test(profile) || profiles.has(profile)) {
      throw new HarnessError(`${prefix}.profile must be a unique harness-* slug`, 'invalid_config');
    }
    profiles.add(profile);
    const controlPort = raw.controlPort;
    if (!Number.isInteger(controlPort) || controlPort < 1024 || controlPort > 65_535) {
      throw new HarnessError(`${prefix}.controlPort is invalid`, 'invalid_config');
    }
    const portKey = `${kind}:${raw.sshTarget ?? 'local'}:${controlPort}`;
    if (ports.has(portKey)) {
      throw new HarnessError(
        `${prefix}.controlPort collides with another endpoint`,
        'invalid_config',
      );
    }
    ports.add(portKey);
    const endpoint = {
      id,
      kind,
      profile,
      displayName: requiredString(raw.displayName ?? `Harness ${id}`, `${prefix}.displayName`),
      dataDir: safeDataDirectory(raw.dataDir, `${prefix}.dataDir`, kind),
      controlPort,
      harborPath: assertPackagedPath(raw.harborPath, `${prefix}.harborPath`),
      harborctlPath: assertPackagedPath(raw.harborctlPath, `${prefix}.harborctlPath`),
      controlToken: secretSource(
        raw.controlToken,
        `${prefix}.controlToken`,
        kind === 'remote-linux-ssh',
        kind,
      ),
      identityPassphrase: secretSource(
        raw.identityPassphrase,
        `${prefix}.identityPassphrase`,
        kind === 'remote-linux-ssh',
        kind,
      ),
      dialAddress:
        raw.dialAddress === undefined
          ? null
          : requiredString(raw.dialAddress, `${prefix}.dialAddress`),
      headlessMedia: raw.headlessMedia === true,
      runtimeEnvironment: runtimeEnvironment(
        raw.runtimeEnvironment,
        `${prefix}.runtimeEnvironment`,
      ),
    };
    if (endpoint.dialAddress && !endpoint.dialAddress.includes('{peerId}')) {
      throw new HarnessError(`${prefix}.dialAddress must contain {peerId}`, 'invalid_config');
    }
    if (endpoint.dialAddress) {
      const unknownPlaceholders = endpoint.dialAddress.match(/\{(?!peerId\}|tcpPort\})[^}]+\}/g);
      if (unknownPlaceholders) {
        throw new HarnessError(
          `${prefix}.dialAddress contains an unsupported placeholder`,
          'invalid_config',
        );
      }
    }
    if (kind === 'windows-powershell') {
      endpoint.powershellPath = requiredString(
        raw.powershellPath ?? 'powershell.exe',
        `${prefix}.powershellPath`,
      );
    }
    if (kind === 'remote-linux-ssh') {
      endpoint.sshPath = requiredString(raw.sshPath ?? 'ssh', `${prefix}.sshPath`);
      endpoint.sshTarget = requiredString(raw.sshTarget, `${prefix}.sshTarget`);
      if (!/^[A-Za-z0-9_.@:-]+$/.test(endpoint.sshTarget)) {
        throw new HarnessError(`${prefix}.sshTarget contains unsafe characters`, 'invalid_config');
      }
      endpoint.sshArgs = Array.isArray(raw.sshArgs)
        ? raw.sshArgs.map((item, itemIndex) =>
            requiredString(item, `${prefix}.sshArgs[${itemIndex}]`),
          )
        : [];
    }
    return endpoint;
  });
  const roleInput = input.roles ?? {};
  const roles = {
    caller: roleInput.caller ?? endpoints[0].id,
    callee: roleInput.callee ?? endpoints[1].id,
    groupPeer: roleInput.groupPeer ?? endpoints[2].id,
  };
  if (new Set(Object.values(roles)).size !== 3 || Object.values(roles).some((id) => !ids.has(id))) {
    throw new HarnessError(
      'roles must select three distinct configured endpoints',
      'invalid_config',
    );
  }
  return {
    schemaVersion: 1,
    endpoints,
    roles,
    commandTimeoutMs: duration(
      input.commandTimeoutMs,
      DEFAULTS.commandTimeoutMs,
      'commandTimeoutMs',
    ),
    convergenceTimeoutMs: duration(
      input.convergenceTimeoutMs,
      DEFAULTS.convergenceTimeoutMs,
      'convergenceTimeoutMs',
    ),
    callTimeoutConvergenceMs: duration(
      input.callTimeoutConvergenceMs,
      DEFAULTS.callTimeoutConvergenceMs,
      'callTimeoutConvergenceMs',
    ),
    pollIntervalMs: duration(input.pollIntervalMs, DEFAULTS.pollIntervalMs, 'pollIntervalMs'),
    cleanupTimeoutMs: duration(
      input.cleanupTimeoutMs,
      DEFAULTS.cleanupTimeoutMs,
      'cleanupTimeoutMs',
    ),
    evidenceFile:
      input.evidenceFile === undefined ? null : requiredString(input.evidenceFile, 'evidenceFile'),
  };
}

export function redactSensitiveText(value, knownSecrets = []) {
  let text = String(value ?? '');
  for (const secret of knownSecrets) {
    if (secret) text = text.split(secret).join('[REDACTED]');
  }
  return text
    .replace(
      /-----BEGIN[\s\S]{0,100}?PRIVATE KEY-----[\s\S]*?-----END[\s\S]{0,100}?PRIVATE KEY-----/gi,
      '[REDACTED_PRIVATE_KEY]',
    )
    .replace(/(?:^|\n)v=0\r?\n[\s\S]{0,12000}?(?=\n\S|$)/gim, '\n[REDACTED_SDP]')
    .replace(/candidate:[^\s"']+(?:\s+[^\s"']+){5,}/gi, '[REDACTED_ICE]')
    .replace(
      /\b(?:ice-pwd|ice-ufrag|fingerprint)\s*[:=]\s*[^\s,"']+/gi,
      '[REDACTED_ICE_CREDENTIAL]',
    )
    .replace(
      /(["']?(?:token|password|passphrase|credential|signature|nonce|privateKey|private_key|sdp|candidate)["']?\s*[:=]\s*)[^,}\n]+/gi,
      '$1[REDACTED]',
    )
    .replace(/\b12D3KooW[1-9A-HJ-NP-Za-km-z]{20,}\b/g, '[PROFILE_ID]');
}

function shellQuote(value) {
  return `'${String(value).replaceAll("'", `'"'"'`)}'`;
}

function psLiteral(value) {
  return `'${String(value).replaceAll("'", "''")}'`;
}

function encodePowerShell(script) {
  return Buffer.from(script, 'utf16le').toString('base64');
}

function readLocalSecret(source, label) {
  let value;
  if (source.env) value = process.env[source.env];
  else value = readFileSync(source.file, 'utf8').replace(/[\r\n]+$/, '');
  if (!value) throw new HarnessError(`${label} secret source is empty`, 'missing_secret');
  return value;
}

function secretEnvironment(source, target, label) {
  if (source.env) return { [target]: readLocalSecret(source, label) };
  return { [`${target}_FILE`]: source.file };
}

function secretShell(source, target) {
  if (source.file) {
    return `export ${target}="$(cat -- ${shellQuote(source.file)})"; test -n "$${target}";`;
  }
  return `test -n "$${source.env}"; export ${target}="$${source.env}";`;
}

function secretPowerShell(source, target) {
  if (source.file) {
    return `$env:${target}=(Get-Content -Raw -LiteralPath ${psLiteral(source.file)}).TrimEnd("\`r","\`n"); if ([string]::IsNullOrEmpty($env:${target})) { throw 'secret file is empty' }`;
  }
  return `$env:${target}=[Environment]::GetEnvironmentVariable(${psLiteral(source.env)}); if ([string]::IsNullOrEmpty($env:${target})) { throw 'secret environment variable is empty' }`;
}

function ephemeralSecretFiles(spec) {
  return [
    ...new Set(
      [spec.controlToken, spec.identityPassphrase]
        .filter((source) => source.file && source.ownership === 'harness-ephemeral')
        .map((source) => source.file),
    ),
  ];
}

function removeLocalEphemeralSecretFiles(spec) {
  for (const path of ephemeralSecretFiles(spec)) {
    rmSync(path, { force: true });
    if (existsSync(path)) {
      throw new HarnessError('ephemeral secret cleanup failed', 'cleanup_failed');
    }
  }
}

function runCaptured(file, args, { env = process.env, timeoutMs, maxBytes = 1024 * 1024 } = {}) {
  return new Promise((resolvePromise, rejectPromise) => {
    const child = spawn(file, args, { env, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '';
    let stderr = '';
    let settled = false;
    const timer = setTimeout(() => {
      child.kill('SIGKILL');
      finish(new HarnessError('host command timed out', 'command_timeout'));
    }, timeoutMs);
    const finish = (error, result) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      if (error) rejectPromise(error);
      else resolvePromise(result);
    };
    const append = (current, chunk) => {
      const next = current + chunk.toString('utf8');
      if (Buffer.byteLength(next) > maxBytes) {
        child.kill('SIGKILL');
        finish(new HarnessError('host command output exceeded its safety bound', 'output_limit'));
      }
      return next;
    };
    child.stdout.on('data', (chunk) => {
      stdout = append(stdout, chunk);
    });
    child.stderr.on('data', (chunk) => {
      stderr = append(stderr, chunk);
    });
    child.once('error', (error) => finish(error));
    child.once('exit', (code, signal) => finish(null, { code, signal, stdout, stderr }));
  });
}

function parseControlResponse(output) {
  let response;
  try {
    response = JSON.parse(output);
  } catch {
    throw new HarnessError('harborctl returned malformed JSON', 'invalid_control_response');
  }
  if (!response || typeof response !== 'object' || typeof response.ok !== 'boolean') {
    throw new HarnessError(
      'harborctl returned an invalid response contract',
      'invalid_control_response',
    );
  }
  return response;
}

class LocalWslAdapter {
  constructor(spec, config) {
    this.spec = spec;
    this.config = config;
    this.child = null;
    this.knownSecrets = [];
  }

  environment(includePassphrase = false) {
    const token = readLocalSecret(this.spec.controlToken, `${this.spec.id} control token`);
    this.knownSecrets.push(token);
    return {
      ...process.env,
      ...this.spec.runtimeEnvironment,
      HARBOR_PROFILE: this.spec.profile,
      HARBOR_DATA_DIR: this.spec.dataDir,
      HARBOR_CONTROL_PORT: String(this.spec.controlPort),
      ...(this.spec.headlessMedia ? { HARBOR_HEADLESS_MEDIA_CAPTURE: '1' } : {}),
      HARBOR_CONTROL_TOKEN: token,
      ...(includePassphrase
        ? secretEnvironment(
            this.spec.identityPassphrase,
            'HARBOR_IDENTITY_PASSPHRASE',
            this.spec.id,
          )
        : {}),
    };
  }

  async launch() {
    rmSync(this.spec.dataDir, { recursive: true, force: true });
    mkdirSync(this.spec.dataDir, { recursive: true, mode: 0o700 });
    chmodSync(this.spec.dataDir, 0o700);
    this.child = spawn(this.spec.harborPath, [], {
      env: this.environment(false),
      detached: true,
      stdio: 'ignore',
    });
    await new Promise((resolvePromise, rejectPromise) => {
      const timer = setTimeout(resolvePromise, 200);
      this.child.once('error', (error) => {
        clearTimeout(timer);
        rejectPromise(error);
      });
    });
    this.child.unref();
  }

  async control(args) {
    const result = await runCaptured(this.spec.harborctlPath, args, {
      env: this.environment(true),
      timeoutMs: this.config.commandTimeoutMs,
    });
    const response = parseControlResponse(result.stdout);
    if (result.code !== 0 && response.ok) {
      throw new HarnessError('harborctl exited unsuccessfully', 'control_process_failed');
    }
    return response;
  }

  async cleanup() {
    let cleanupError = null;
    try {
      await Promise.race([this.control(['network-stop']), sleep(2_000)]);
    } catch {}
    try {
      await Promise.race([this.control(['shutdown']), sleep(2_000)]);
    } catch {}
    if (this.child?.pid) {
      try {
        process.kill(-this.child.pid, 'SIGTERM');
      } catch {}
    }
    try {
      rmSync(this.spec.dataDir, { recursive: true, force: true });
      if (existsSync(this.spec.dataDir)) throw new HarnessError('local profile cleanup failed');
    } catch (error) {
      cleanupError = error;
    }
    try {
      removeLocalEphemeralSecretFiles(this.spec);
    } catch (error) {
      cleanupError ??= error;
    }
    if (cleanupError) throw cleanupError;
  }
}

class WindowsPowerShellAdapter {
  constructor(spec, config) {
    this.spec = spec;
    this.config = config;
  }

  async powershellResult(script, timeoutMs = this.config.commandTimeoutMs) {
    return runCaptured(
      this.spec.powershellPath,
      ['-NoProfile', '-NonInteractive', '-EncodedCommand', encodePowerShell(script)],
      { timeoutMs },
    );
  }

  async powershell(script, timeoutMs = this.config.commandTimeoutMs) {
    const result = await this.powershellResult(script, timeoutMs);
    if (result.code !== 0) throw new HarnessError(result.stderr || 'PowerShell command failed');
    return result.stdout.trim();
  }

  async launch() {
    const pidFile = `${this.spec.dataDir}\\.harness.pid`;
    const script = `
$ErrorActionPreference='Stop'
if (Test-Path -LiteralPath ${psLiteral(this.spec.dataDir)}) { Remove-Item -Recurse -Force -LiteralPath ${psLiteral(this.spec.dataDir)} }
New-Item -ItemType Directory -Force -Path ${psLiteral(this.spec.dataDir)} | Out-Null
${secretPowerShell(this.spec.controlToken, 'HARBOR_CONTROL_TOKEN')}
${Object.entries(this.spec.runtimeEnvironment)
  .map(([name, value]) => `$env:${name}=${psLiteral(value)}`)
  .join('\n')}
$env:HARBOR_PROFILE=${psLiteral(this.spec.profile)}
$env:HARBOR_DATA_DIR=${psLiteral(this.spec.dataDir)}
$env:HARBOR_CONTROL_PORT=${psLiteral(String(this.spec.controlPort))}
${this.spec.headlessMedia ? "$env:HARBOR_HEADLESS_MEDIA_CAPTURE='1'" : ''}
$process=$null
try {
  # Start-Process keeps the parent PowerShell alive while a GUI child owns its
  # redirected handles. Use the shell-backed .NET launcher so PowerShell exits
  # immediately and the harness can continue to the control-plane readiness
  # check. Harbor writes its normal bounded application log inside dataDir.
  $startInfo=New-Object System.Diagnostics.ProcessStartInfo
  $startInfo.FileName=${psLiteral(this.spec.harborPath)}
  $startInfo.UseShellExecute=$true
  $process=[System.Diagnostics.Process]::Start($startInfo)
  if (-not $process) { throw 'Harbor process did not start' }
  Set-Content -NoNewline -LiteralPath ${psLiteral(pidFile)} -Value $process.Id
} catch {
  if ($process) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
  throw
}`;
    await this.powershell(script);
  }

  async control(args) {
    const controlArgs = [...args];
    let payloadSetup = '';
    let payloadCleanup = '';
    if (controlArgs[0] === 'frontend' && controlArgs.length === 3) {
      // Windows PowerShell 5 rewrites quotes in native-process arguments. Pass
      // structured payloads through harborctl's @file surface so peer IDs and
      // arrays arrive as exact JSON instead of relying on shell quoting.
      const payloadPath = `${this.spec.dataDir}\\.harness-control-${randomUUID()}.json`;
      const payloadBase64 = Buffer.from(controlArgs[2], 'utf8').toString('base64');
      payloadSetup = `[IO.File]::WriteAllBytes(${psLiteral(payloadPath)}, [Convert]::FromBase64String(${psLiteral(payloadBase64)}))`;
      payloadCleanup = `Remove-Item -Force -LiteralPath ${psLiteral(payloadPath)} -ErrorAction SilentlyContinue`;
      controlArgs[2] = `@${payloadPath}`;
    }
    const argumentList = controlArgs.map(psLiteral).join(',');
    const script = `
$ErrorActionPreference='Stop'
${secretPowerShell(this.spec.controlToken, 'HARBOR_CONTROL_TOKEN')}
${secretPowerShell(this.spec.identityPassphrase, 'HARBOR_IDENTITY_PASSPHRASE')}
${Object.entries(this.spec.runtimeEnvironment)
  .map(([name, value]) => `$env:${name}=${psLiteral(value)}`)
  .join('\n')}
$env:HARBOR_CONTROL_PORT=${psLiteral(String(this.spec.controlPort))}
${payloadSetup}
try {
  & ${psLiteral(this.spec.harborctlPath)} @(${argumentList})
  $harborctlExitCode=$LASTEXITCODE
} finally {
  ${payloadCleanup}
}
if ($harborctlExitCode -ne 0) { exit $harborctlExitCode }`;
    const result = await this.powershellResult(script);
    const response = parseControlResponse(result.stdout.trim());
    if (result.code !== 0 && response.ok) {
      throw new HarnessError('harborctl exited unsuccessfully', 'control_process_failed');
    }
    return response;
  }

  async cleanup() {
    try {
      await Promise.race([this.control(['network-stop']), sleep(2_000)]);
    } catch {}
    try {
      await Promise.race([this.control(['shutdown']), sleep(2_000)]);
    } catch {}
    const pidFile = `${this.spec.dataDir}\\.harness.pid`;
    const ephemeralFiles = ephemeralSecretFiles(this.spec);
    const removeEphemeralFiles = ephemeralFiles
      .map((path) => `Remove-Item -Force -LiteralPath ${psLiteral(path)}`)
      .join('\n');
    const verifyEphemeralFiles = ephemeralFiles
      .map(
        (path) =>
          `if (Test-Path -LiteralPath ${psLiteral(path)}) { throw 'ephemeral secret cleanup failed' }`,
      )
      .join('\n');
    const script = `
$ErrorActionPreference='SilentlyContinue'
if (Test-Path -LiteralPath ${psLiteral(pidFile)}) { $id=Get-Content -Raw -LiteralPath ${psLiteral(pidFile)}; Stop-Process -Id $id -Force }
Remove-Item -Recurse -Force -LiteralPath ${psLiteral(this.spec.dataDir)}
${removeEphemeralFiles}
if (Test-Path -LiteralPath ${psLiteral(this.spec.dataDir)}) { throw 'profile cleanup failed' }
${verifyEphemeralFiles}`;
    await this.powershell(script, this.config.cleanupTimeoutMs);
  }
}

class RemoteLinuxSshAdapter {
  constructor(spec, config) {
    this.spec = spec;
    this.config = config;
  }

  async sshResult(script, timeoutMs = this.config.commandTimeoutMs) {
    const remoteCommand = `sh -lc ${shellQuote(script)}`;
    return runCaptured(
      this.spec.sshPath,
      [...this.spec.sshArgs, this.spec.sshTarget, remoteCommand],
      { timeoutMs },
    );
  }

  async ssh(script, timeoutMs = this.config.commandTimeoutMs) {
    const result = await this.sshResult(script, timeoutMs);
    if (result.code !== 0) throw new HarnessError(result.stderr || 'SSH command failed');
    return result.stdout.trim();
  }

  async launch() {
    const appEnvironment = [
      `HARBOR_PROFILE=${shellQuote(this.spec.profile)}`,
      `HARBOR_DATA_DIR=${shellQuote(this.spec.dataDir)}`,
      `HARBOR_CONTROL_PORT=${shellQuote(String(this.spec.controlPort))}`,
      ...(this.spec.headlessMedia ? ['HARBOR_HEADLESS_MEDIA_CAPTURE=1'] : []),
      ...Object.entries(this.spec.runtimeEnvironment).map(
        ([name, value]) => `${name}=${shellQuote(value)}`,
      ),
    ].join(' ');
    const script = `set -eu
rm -rf -- ${shellQuote(this.spec.dataDir)}
mkdir -m 700 -p -- ${shellQuote(this.spec.dataDir)}
${secretShell(this.spec.controlToken, 'HARBOR_CONTROL_TOKEN')}
nohup env ${appEnvironment} ${shellQuote(this.spec.harborPath)} >${shellQuote(`${this.spec.dataDir}/.harness.stdout.log`)} 2>${shellQuote(`${this.spec.dataDir}/.harness.stderr.log`)} &
pid=$!
printf '%s' "$pid" >${shellQuote(`${this.spec.dataDir}/.harness.pid`)}`;
    await this.ssh(script);
  }

  async control(args) {
    const script = `set -eu
${secretShell(this.spec.controlToken, 'HARBOR_CONTROL_TOKEN')}
${secretShell(this.spec.identityPassphrase, 'HARBOR_IDENTITY_PASSPHRASE')}
${Object.entries(this.spec.runtimeEnvironment)
  .map(([name, value]) => `export ${name}=${shellQuote(value)}`)
  .join('\n')}
export HARBOR_CONTROL_PORT=${shellQuote(String(this.spec.controlPort))}
exec ${shellQuote(this.spec.harborctlPath)} ${args.map(shellQuote).join(' ')}`;
    const result = await this.sshResult(script);
    const response = parseControlResponse(result.stdout.trim());
    if (result.code !== 0 && response.ok) {
      throw new HarnessError('harborctl exited unsuccessfully', 'control_process_failed');
    }
    return response;
  }

  async cleanup() {
    try {
      await Promise.race([this.control(['network-stop']), sleep(2_000)]);
    } catch {}
    try {
      await Promise.race([this.control(['shutdown']), sleep(2_000)]);
    } catch {}
    const ephemeralFiles = ephemeralSecretFiles(this.spec);
    const removeEphemeralFiles = ephemeralFiles.map(shellQuote).join(' ');
    const verifyEphemeralFiles = ephemeralFiles
      .map((path) => `test ! -e ${shellQuote(path)} || cleanup_failed=1`)
      .join('\n');
    const script = `set +e
if test -f ${shellQuote(`${this.spec.dataDir}/.harness.pid`)}; then kill "$(cat ${shellQuote(`${this.spec.dataDir}/.harness.pid`)})" 2>/dev/null; fi
rm -rf -- ${shellQuote(this.spec.dataDir)}
${removeEphemeralFiles ? `rm -f -- ${removeEphemeralFiles}` : ''}
cleanup_failed=0
test ! -e ${shellQuote(this.spec.dataDir)} || cleanup_failed=1
${verifyEphemeralFiles}
exit "$cleanup_failed"`;
    await this.ssh(script, this.config.cleanupTimeoutMs);
  }
}

export function createRealAdapter(spec, config) {
  if (spec.kind === 'local-wsl') return new LocalWslAdapter(spec, config);
  if (spec.kind === 'windows-powershell') return new WindowsPowerShellAdapter(spec, config);
  return new RemoteLinuxSshAdapter(spec, config);
}

class EvidenceRecorder {
  constructor(config) {
    this.startedAt = new Date().toISOString();
    this.events = [];
    this.scenarios = [];
    this.config = config;
  }

  event(scenario, step, endpoint = null, state = null) {
    const safeState = SAFE_CALL_STATES.has(state) || SAFE_GROUP_STATES.has(state) ? state : null;
    this.events.push({ at: new Date().toISOString(), scenario, step, endpoint, state: safeState });
  }

  scenario(id, outcome) {
    this.scenarios.push({ id, outcome });
  }

  result(outcome, cleanup, failure = null) {
    return {
      schemaVersion: 1,
      runId: randomUUID(),
      startedAt: this.startedAt,
      finishedAt: new Date().toISOString(),
      outcome,
      endpoints: this.config.endpoints.map(({ id, kind }) => ({ id, kind })),
      scenarios: this.scenarios,
      events: this.events,
      cleanup,
      failure: failure
        ? { code: failure.code ?? 'harness_failed', message: redactSensitiveText(failure.message) }
        : null,
    };
  }
}

function sleep(milliseconds) {
  return new Promise((resolvePromise) => setTimeout(resolvePromise, milliseconds));
}

async function invoke(adapter, args) {
  const response = await adapter.control(args);
  if (!response.ok) {
    throw new HarnessError(
      redactSensitiveText(response.error ?? 'control action failed'),
      'control_failed',
    );
  }
  return response.result ?? {};
}

async function waitFor(label, read, predicate, config, timeoutMs = config.convergenceTimeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let latest;
  let lastReadFailure = null;
  while (Date.now() <= deadline) {
    try {
      latest = await read();
      lastReadFailure = null;
      if (predicate(latest)) return latest;
    } catch (error) {
      lastReadFailure = error instanceof HarnessError ? error.code : 'read_failed';
    }
    await sleep(config.pollIntervalMs);
  }
  const failure = new HarnessError(
    `${label} did not converge before its deadline`,
    'convergence_timeout',
  );
  failure.latest = latest;
  failure.lastReadFailure = lastReadFailure;
  throw failure;
}

async function snapshot(adapter) {
  return invoke(adapter, ['frontend', 'state.snapshot']);
}

function callState(value) {
  return typeof value?.call?.state === 'string' ? value.call.state : 'invalid';
}

function groupState(value) {
  return typeof value?.group?.state === 'string' ? value.group.state : 'invalid';
}

function safeCallDiagnostic(value) {
  const state = SAFE_CALL_STATES.has(callState(value)) ? callState(value) : 'invalid';
  const reason = SAFE_CALL_TERMINAL_REASONS.has(value?.call?.terminalReason)
    ? value.call.terminalReason
    : 'none';
  const ice = SAFE_ICE_STATES.has(value?.call?.ice?.iceConnectionState)
    ? value.call.ice.iceConnectionState
    : 'none';
  const connection = SAFE_ICE_STATES.has(value?.call?.ice?.connectionState)
    ? value.call.ice.connectionState
    : 'none';
  return `${state} (reason=${reason}, ice=${ice}, connection=${connection})`;
}

function safeGroupDiagnostic(value) {
  const state = SAFE_GROUP_STATES.has(groupState(value)) ? groupState(value) : 'invalid';
  const participantStates = Array.isArray(value?.group?.participants)
    ? value.group.participants.map((participant) =>
        SAFE_GROUP_PARTICIPANT_STATES.has(participant?.state) ? participant.state : 'invalid',
      )
    : [];
  return `${state} (participants=${participantStates.join(',') || 'none'})`;
}

async function waitCall(adapter, states, label, config, recorder, scenario, timeoutMs) {
  const accepted = new Set(states);
  let result;
  try {
    result = await waitFor(
      label,
      () => snapshot(adapter),
      (value) => accepted.has(callState(value)),
      config,
      timeoutMs,
    );
  } catch (error) {
    const latest = error?.latest ?? null;
    const readFailure =
      typeof error?.lastReadFailure === 'string' ? `; snapshot=${error.lastReadFailure}` : '';
    throw new HarnessError(
      `${label} observed ${safeCallDiagnostic(latest)} instead of ${[...accepted].join(' or ')}${readFailure}`,
      error?.code ?? 'convergence_timeout',
    );
  }
  recorder.event(scenario, label, adapter.spec.id, callState(result));
  return result;
}

async function waitGroup(adapter, states, label, config, recorder, scenario) {
  const accepted = new Set(states);
  let result;
  try {
    result = await waitFor(
      label,
      () => snapshot(adapter),
      (value) => accepted.has(groupState(value)),
      config,
    );
  } catch (error) {
    const latest = await snapshot(adapter).catch(() => null);
    throw new HarnessError(
      `${label} observed ${SAFE_GROUP_STATES.has(groupState(latest)) ? groupState(latest) : 'invalid'} instead of ${[...accepted].join(' or ')}`,
      error?.code ?? 'convergence_timeout',
    );
  }
  recorder.event(scenario, label, adapter.spec.id, groupState(result));
  return result;
}

async function acceptedCall(caller, callee, video, scenario, config, recorder) {
  await invoke(caller, [
    'frontend',
    'call.start',
    JSON.stringify({ peerId: callee.peerId, video }),
  ]);
  let started;
  try {
    started = await waitCall(
      caller,
      ['requesting_microphone', 'connecting', 'ringing', 'failed'],
      'caller-started',
      config,
      recorder,
      scenario,
    );
  } catch (error) {
    const backend = await invoke(caller, ['status']).then(
      () => 'reachable',
      () => 'unreachable',
    );
    throw new HarnessError(
      `${error instanceof Error ? error.message : 'caller startup failed'}; backend=${backend}`,
      error?.code ?? 'call_start_failed',
    );
  }
  if (callState(started) === 'failed') {
    throw new HarnessError(
      `${scenario} caller failed during startup: ${safeCallDiagnostic(started)}`,
      'call_start_failed',
    );
  }
  await waitCall(callee, ['incoming'], 'incoming', config, recorder, scenario);
  await invoke(callee, ['frontend', 'call.accept']);
  await Promise.all([
    waitCall(caller, ['connected'], 'caller-connected', config, recorder, scenario),
    waitCall(callee, ['connected'], 'callee-connected', config, recorder, scenario),
  ]);
  await invoke(caller, ['frontend', 'call.hangup']);
  await Promise.all([
    waitCall(caller, [...TERMINAL_CALL_STATES], 'caller-terminal', config, recorder, scenario),
    waitCall(callee, [...TERMINAL_CALL_STATES], 'callee-terminal', config, recorder, scenario),
  ]);
  recorder.scenario(scenario, 'pass');
}

async function timeoutLateAnswer(caller, callee, config, recorder) {
  const scenario = 'timeout-late-answer';
  await invoke(caller, [
    'frontend',
    'call.start',
    JSON.stringify({ peerId: callee.peerId, video: false }),
  ]);
  await waitCall(callee, ['incoming'], 'incoming', config, recorder, scenario);
  const terminal = await waitCall(
    caller,
    ['ended', 'failed'],
    'caller-timeout',
    config,
    recorder,
    scenario,
    config.callTimeoutConvergenceMs,
  );
  if (terminal.call.terminalReason !== 'timeout') {
    throw new HarnessError('timed call ended without the timeout terminal reason');
  }
  try {
    await invoke(callee, ['frontend', 'call.accept']);
  } catch (error) {
    recorder.event(scenario, 'late-answer-rejected', callee.spec.id);
  }
  const late = await snapshot(callee);
  if (callState(late) === 'connected') {
    throw new HarnessError('a late answer reconnected an expired call');
  }
  recorder.event(scenario, 'late-answer-not-connected', callee.spec.id, callState(late));
  await invoke(callee, ['frontend', 'call.hangup']).catch(() => {});
  recorder.scenario(scenario, 'pass');
}

async function groupPartialFailure(caller, callee, groupPeer, config, recorder) {
  const scenario = 'group-partial-failure';
  await invoke(caller, [
    'frontend',
    'group.start',
    JSON.stringify({ peerIds: [callee.peerId, groupPeer.peerId], video: true }),
  ]);
  await Promise.all([
    waitGroup(callee, ['ringing'], 'callee-invited', config, recorder, scenario),
    waitGroup(groupPeer, ['ringing'], 'group-peer-invited', config, recorder, scenario),
  ]);
  await invoke(callee, ['frontend', 'group.accept']);
  await invoke(groupPeer, ['frontend', 'group.accept']);
  try {
    await waitFor(
      'group mesh connection',
      () => snapshot(caller),
      (value) => {
        const participants = Array.isArray(value.group?.participants)
          ? value.group.participants
          : [];
        return (
          groupState(value) === 'connected' &&
          participants.length === 2 &&
          participants.every((participant) => participant.state === 'connected')
        );
      },
      config,
    );
  } catch (error) {
    const latest = await snapshot(caller).catch(() => null);
    throw new HarnessError(
      `group mesh connection observed ${safeGroupDiagnostic(latest)}`,
      error?.code ?? 'convergence_timeout',
    );
  }
  recorder.event(scenario, 'mesh-connected', caller.spec.id, 'connected');
  await invoke(groupPeer, ['network-stop']);
  groupPeer.networkStoppedForScenario = true;
  let degraded;
  try {
    degraded = await waitFor(
      'group partial failure',
      () => snapshot(caller),
      (value) => {
        if (!['degraded', 'connected'].includes(groupState(value))) return false;
        const participants = Array.isArray(value.group?.participants)
          ? value.group.participants
          : [];
        const healthy = participants.some(
          (participant) =>
            participant.peerId === callee.peerId && participant.state === 'connected',
        );
        const failed = participants.some(
          (participant) =>
            participant.peerId === groupPeer.peerId &&
            ['failed', 'degraded', 'disconnected', 'timed_out', 'left'].includes(participant.state),
        );
        return healthy && failed;
      },
      config,
    );
  } catch (error) {
    const latest = await snapshot(caller).catch(() => null);
    throw new HarnessError(
      `group partial failure observed ${safeGroupDiagnostic(latest)}`,
      error?.code ?? 'convergence_timeout',
    );
  }
  recorder.event(scenario, 'remaining-leg-connected', caller.spec.id, groupState(degraded));
  await invoke(caller, ['frontend', 'group.leave']);
  await Promise.all([
    waitGroup(caller, ['idle', 'ended'], 'caller-group-cleared', config, recorder, scenario),
    waitGroup(callee, ['idle', 'ended'], 'callee-group-cleared', config, recorder, scenario),
  ]);
  recorder.scenario(scenario, 'pass');
}

async function provision(adapters, config, recorder) {
  for (const adapter of adapters) {
    await adapter.launch();
    recorder.event('provision', 'process-launched', adapter.spec.id);
  }
  for (const adapter of adapters) {
    await waitFor(
      `${adapter.spec.id} control plane`,
      () => invoke(adapter, ['status']),
      () => true,
      config,
    );
    const created = await invoke(adapter, ['identity-create', adapter.spec.displayName]);
    adapter.peerId = created.peerId;
    if (typeof adapter.peerId !== 'string' || adapter.peerId.length < 16) {
      throw new HarnessError(`${adapter.spec.id} did not return a valid profile identifier`);
    }
    await waitFor(
      `${adapter.spec.id} frontend identity refresh`,
      () => invoke(adapter, ['frontend', 'identity.refresh']),
      (value) => value?.status === 'unlocked',
      config,
    );
    await waitFor(
      `${adapter.spec.id} profile event listeners`,
      () => snapshot(adapter),
      (value) => value?.identity?.status === 'unlocked' && value?.profileEventsReady === true,
      config,
    );
    await invoke(adapter, ['network-start']);
    recorder.event('provision', 'profile-ready', adapter.spec.id);
  }

  for (const target of adapters) {
    if (!target.spec.dialAddress) continue;
    let address = target.spec.dialAddress.replaceAll('{peerId}', target.peerId);
    if (address.includes('{tcpPort}')) {
      const listeningAddresses = await invoke(target, ['network-addresses']);
      const tcpAddress = Array.isArray(listeningAddresses)
        ? listeningAddresses.find(
            (candidate) =>
              typeof candidate === 'string' &&
              /\/tcp\/\d+/.test(candidate) &&
              !candidate.includes('/p2p-circuit') &&
              candidate.endsWith(`/p2p/${target.peerId}`),
          )
        : null;
      const tcpPort = tcpAddress?.match(/\/tcp\/(\d+)/)?.[1];
      if (!tcpPort) {
        throw new HarnessError(
          `${target.spec.id} did not expose a TCP listening port for its dial template`,
          'invalid_control_response',
        );
      }
      address = address.replaceAll('{tcpPort}', tcpPort);
    }
    for (const source of adapters) {
      if (source !== target) await invoke(source, ['network-connect', address]);
    }
  }
  for (const adapter of adapters) {
    const expected = adapters.filter((candidate) => candidate !== adapter);
    try {
      await waitFor(
        `${adapter.spec.id} peer connectivity`,
        () => invoke(adapter, ['network-peers']),
        (peers) =>
          expected.every((candidate) =>
            Array.isArray(peers)
              ? peers.some((peer) => peer.peerId === candidate.peerId && peer.isConnected === true)
              : false,
          ),
        config,
      );
    } catch (error) {
      const peers = await invoke(adapter, ['network-peers']).catch(() => []);
      const missing = expected
        .filter(
          (candidate) =>
            !Array.isArray(peers) ||
            !peers.some((peer) => peer.peerId === candidate.peerId && peer.isConnected === true),
        )
        .map((candidate) => candidate.spec.id);
      throw new HarnessError(
        `${adapter.spec.id} peer connectivity is missing: ${missing.join(', ') || 'unknown endpoint'}`,
        error?.code ?? 'convergence_timeout',
      );
    }
    recorder.event('provision', 'peers-connected', adapter.spec.id);
  }

  for (let left = 0; left < adapters.length; left += 1) {
    for (let right = left + 1; right < adapters.length; right += 1) {
      const requester = adapters[left];
      const receiver = adapters[right];
      await invoke(requester, ['contact-request', receiver.peerId]);
      await waitFor(
        `${receiver.spec.id} incoming contact request`,
        () =>
          invoke(receiver, ['contact-accept', requester.peerId])
            .then(() => true)
            .catch(() => false),
        Boolean,
        config,
      );
      await Promise.all(
        [
          [requester, receiver.peerId],
          [receiver, requester.peerId],
        ].map(([adapter, peerId]) =>
          waitFor(
            `${adapter.spec.id} contact grants`,
            () => invoke(adapter, ['contact-status', peerId]),
            (status) =>
              status.isContact === true &&
              status.issuedCallGrant === true &&
              status.receivedCallGrant === true,
            config,
          ),
        ),
      );
    }
  }
  recorder.event('provision', 'contacts-and-grants-converged');
  recorder.scenario('provision', 'pass');
  // Let the just-completed contact/grant event burst drain before the first
  // call offer. Real users naturally introduce this gap while navigating;
  // automation otherwise sends the offer in the same event-loop turn.
  await sleep(Math.max(250, config.pollIntervalMs * 2));
}

async function cleanupAll(adapters, config) {
  const results = [];
  for (const adapter of [...adapters].reverse()) {
    let outcome = 'complete';
    try {
      await Promise.race([
        adapter.cleanup(),
        sleep(config.cleanupTimeoutMs).then(() => {
          throw new HarnessError('cleanup timed out', 'cleanup_timeout');
        }),
      ]);
    } catch {
      outcome = 'failed';
    }
    results.push({ endpoint: adapter.spec.id, outcome });
  }
  return results.reverse();
}

export async function runCallHarness(input, options = {}) {
  const config = validateHarnessConfig(input);
  const recorder = new EvidenceRecorder(config);
  const adapterFactory = options.adapterFactory ?? createRealAdapter;
  const adapters = config.endpoints.map((spec) => adapterFactory(spec, config));
  const byId = new Map(adapters.map((adapter) => [adapter.spec.id, adapter]));
  const caller = byId.get(config.roles.caller);
  const callee = byId.get(config.roles.callee);
  const groupPeer = byId.get(config.roles.groupPeer);
  let failure = null;
  let cleanup = [];
  try {
    await provision(adapters, config, recorder);
    await acceptedCall(caller, callee, false, 'voice', config, recorder);
    await acceptedCall(callee, caller, false, 'reverse-voice', config, recorder);
    await acceptedCall(caller, callee, true, 'video', config, recorder);
    await acceptedCall(caller, groupPeer, false, 'arm64-voice', config, recorder);
    await groupPartialFailure(caller, callee, groupPeer, config, recorder);
    await sleep(Math.max(250, config.pollIntervalMs * 2));
    await acceptedCall(caller, callee, false, 'call-after-group', config, recorder);
    await timeoutLateAnswer(caller, callee, config, recorder);
  } catch (error) {
    failure = error instanceof Error ? error : new HarnessError(String(error));
  } finally {
    cleanup = await cleanupAll(adapters, config);
  }
  const outcome = failure || cleanup.some((item) => item.outcome !== 'complete') ? 'fail' : 'pass';
  const result = recorder.result(outcome, cleanup, failure);
  if (config.evidenceFile && options.writeEvidence !== false) {
    const outputPath = resolve(config.evidenceFile);
    mkdirSync(dirname(outputPath), { recursive: true });
    writeFileSync(outputPath, `${JSON.stringify(result, null, 2)}\n`, {
      encoding: 'utf8',
      mode: 0o600,
    });
  }
  return result;
}

function usage() {
  process.stderr.write(`Usage:
  node scripts/cross-host-call-harness.mjs check --config <config.json>
  node scripts/cross-host-call-harness.mjs run --config <config.json> --execute-real-hosts

The run command will not launch anything without the exact --execute-real-hosts opt-in flag.
`);
}

function parseCli(args) {
  const command = args[0];
  const configIndex = args.indexOf('--config');
  if (!['check', 'run'].includes(command) || configIndex < 0 || !args[configIndex + 1]) {
    throw new HarnessError('invalid command line', 'usage');
  }
  const expected =
    command === 'check'
      ? [command, '--config', args[configIndex + 1]]
      : [command, '--config', args[configIndex + 1], '--execute-real-hosts'];
  if (args.length !== expected.length || expected.some((value, index) => args[index] !== value)) {
    throw new HarnessError('unknown command-line argument', 'usage');
  }
  return {
    command,
    configPath: args[configIndex + 1],
    execute: args.includes('--execute-real-hosts'),
  };
}

async function main() {
  try {
    const cli = parseCli(process.argv.slice(2));
    const configPath = resolve(cli.configPath);
    if (!existsSync(configPath)) throw new HarnessError('configuration file does not exist');
    const config = JSON.parse(readFileSync(configPath, 'utf8'));
    const validated = validateHarnessConfig(config);
    if (cli.command === 'check') {
      process.stdout.write(
        `${JSON.stringify({ ok: true, endpoints: validated.endpoints.map(({ id, kind }) => ({ id, kind })) })}\n`,
      );
      return;
    }
    if (!cli.execute) {
      throw new HarnessError(
        'real host execution requires --execute-real-hosts',
        'execution_not_opted_in',
      );
    }
    const result = await runCallHarness(config);
    process.stdout.write(
      `${JSON.stringify({ outcome: result.outcome, scenarios: result.scenarios, cleanup: result.cleanup })}\n`,
    );
    if (result.outcome !== 'pass') process.exitCode = 1;
  } catch (error) {
    usage();
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`${redactSensitiveText(message)}\n`);
    process.exitCode = 2;
  }
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  await main();
}
