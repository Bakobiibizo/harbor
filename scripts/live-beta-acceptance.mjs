#!/usr/bin/env node

import {
  closeSync,
  existsSync,
  lstatSync,
  mkdirSync,
  openSync,
  readFileSync,
  readSync,
  realpathSync,
  renameSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { createHash, randomUUID } from 'node:crypto';
import { dirname, extname, isAbsolute, join, relative, resolve, sep } from 'node:path';
import process from 'node:process';

const SCHEMA_VERSION = 2;
const SCENARIOS = [
  ['P00', 'Automated, packaged-artifact, and production deployment prerequisites', 'P0', 4],
  ['R01', 'Returning identity and restart', 'P0', 2],
  ['R02', 'Password confirmation, keyboard, and lock warning', 'P0', 2],
  ['R03', 'Browser handoff and install fallback', 'P0', 2],
  ['R04', 'HTTPS and harbor invite normalization', 'P0', 2],
  ['R05', 'Contact request lifecycle and notification', 'P0', 2],
  ['R06', 'Authorized contact wall and privacy denial', 'P0', 3],
  ['R07', 'Reactive post, contact, and message refresh', 'P0', 2],
  ['R08', 'Media transfer states and retry', 'P0', 2],
  ['R09', 'Offline feed catch-up and bounded media prefetch', 'P0', 2],
  ['R10', 'Foreground, background, and missed notifications', 'P0', 2],
  ['R11A', 'Windows to macOS voice calls in both directions', 'P0', 2],
  ['R11B', 'Windows to macOS video calls in both directions', 'P0', 2],
  ['R11C', 'Three-profile group call and partial failure', 'P0', 2],
  ['R12', 'Safe link cards and consent-gated embeds', 'P0', 2],
  ['R13', 'Composer, feed filters, onboarding, and bug tracking', 'P0', 2],
  ['R14', 'Pointer, keyboard, focus, and reduced-motion interaction', 'P0', 2],
  ['R15', 'Verified names replace keys on normal surfaces', 'P0', 2],
];

const METADATA = {
  sessionId: {
    defaultValue: () => randomUUID(),
    validate: (value) =>
      /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value),
  },
  startedAtUtc: { defaultValue: () => new Date().toISOString(), validate: validIsoDate },
  commit: { defaultValue: '', validate: (value) => /^[0-9a-f]{40}$/i.test(value) },
  version: {
    defaultValue: '',
    validate: (value) => /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(value),
  },
  productionDownloadUrl: { defaultValue: '', validate: validProductionUrl },
  windowsVersion: {
    defaultValue: '',
    validate: (value) => /windows/i.test(value) && value.trim().length >= 10,
  },
  windowsArchitecture: { defaultValue: '', validate: validArchitecture },
  windowsPackage: { defaultValue: '', validate: (value) => /\.(?:msi|exe)$/i.test(value.trim()) },
  windowsPackageSha256: { defaultValue: '', validate: validSha256 },
  windowsSignatureStatus: { defaultValue: '', validate: (value) => value === 'valid' },
  macosVersion: {
    defaultValue: '',
    validate: (value) => /macos/i.test(value) && value.trim().length >= 8,
  },
  macosArchitecture: { defaultValue: '', validate: validArchitecture },
  macosPackage: {
    defaultValue: '',
    validate: (value) => /\.(?:dmg|pkg|zip|app)$/i.test(value.trim()),
  },
  macosPackageSha256: { defaultValue: '', validate: validSha256 },
  macosCodeSignStatus: { defaultValue: '', validate: (value) => value === 'valid' },
  macosGatekeeperStatus: { defaultValue: '', validate: (value) => value === 'accepted' },
  macosNotarizationStatus: { defaultValue: '', validate: (value) => value === 'accepted' },
  thirdProfilePlatform: {
    defaultValue: '',
    validate: (value) => /^(?:windows|macos)$/i.test(value.trim()),
  },
  thirdProfileArchitecture: { defaultValue: '', validate: validArchitecture },
  relayArtifact: { defaultValue: '', validate: nonEmpty },
  relayArtifactSha256: { defaultValue: '', validate: validSha256 },
  relayNamespace: { defaultValue: 'harbor.social', validate: (value) => value === 'harbor.social' },
  operator: { defaultValue: '', validate: (value) => value.trim().length >= 2 },
  evidenceRedactionAttestation: {
    defaultValue: '',
    validate: (value) => value === 'confirmed',
  },
};

const VALID_OUTCOMES = new Set(['not_run', 'pass', 'fail', 'blocked']);
const DEFAULT_DIR = 'artifacts/live-beta-acceptance';
const TEXT_EVIDENCE_EXTENSIONS = new Set(['.txt', '.log', '.md', '.json', '.csv']);
const SENSITIVE_TEXT_PATTERNS = [
  ['private key', /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/i],
  ['authorization token', /\bBearer\s+[A-Za-z0-9._~+/=-]{12,}/i],
  [
    'secret field',
    /\b(?:password|passphrase|recovery(?: phrase)?|mnemonic|private[_ -]?key|turn[_ -]?credential)\s*[:=]\s*(?!\[?redacted\]?|removed\b)\S+/i,
  ],
  ['session description', /(?:^|\n)v=0\r?\n[\s\S]{0,4000}?\nm=(?:audio|video)\s/im],
  [
    'ICE candidate body',
    /candidate:\S+\s+\d+\s+(?:udp|tcp)\s+\d+\s+(?:(?:\d{1,3}\.){3}\d{1,3}|[0-9a-f:]{3,})\s+\d+/i,
  ],
  ['raw peer ID', /\b12D3KooW[1-9A-HJ-NP-Za-km-z]{20,}\b/],
  [
    'private network address',
    /\b(?:10(?:\.\d{1,3}){3}|192\.168(?:\.\d{1,3}){2}|172\.(?:1[6-9]|2\d|3[01])(?:\.\d{1,3}){2})\b/,
  ],
];

function nonEmpty(value) {
  return value.trim().length > 0;
}

function validIsoDate(value) {
  return /^\d{4}-\d{2}-\d{2}T/.test(value) && Number.isFinite(Date.parse(value));
}

function validProductionUrl(value) {
  try {
    const url = new URL(value);
    return (
      url.protocol === 'https:' &&
      (url.hostname === 'social-harbor.com' || url.hostname === 'www.social-harbor.com')
    );
  } catch {
    return false;
  }
}

function validArchitecture(value) {
  return /^(?:x86_64|amd64|arm64|aarch64)$/i.test(value.trim());
}

function validSha256(value) {
  return /^[0-9a-f]{64}$/i.test(value.trim());
}

function usage(exitCode = 0) {
  const output = exitCode === 0 ? process.stdout : process.stderr;
  output.write(`Usage:
  pnpm acceptance:live-beta init [--dir <directory>]
  pnpm acceptance:live-beta record <scenario> <pass|fail|blocked> <note> <evidence ...> [--dir <directory>]
  pnpm acceptance:live-beta metadata <field> <value> [--dir <directory>]
  pnpm acceptance:live-beta check [--dir <directory>]

All values and paths containing spaces must be quoted in PowerShell, Command Prompt, zsh, and bash.
Metadata fields: ${Object.keys(METADATA).join(', ')}
`);
  process.exit(exitCode);
}

function parseDirectory(args) {
  const remaining = [];
  let directory = DEFAULT_DIR;
  let hasExplicitDirectory = false;
  for (let index = 0; index < args.length; index += 1) {
    if (args[index] !== '--dir') {
      remaining.push(args[index]);
      continue;
    }
    if (hasExplicitDirectory || !args[index + 1] || args[index + 1].startsWith('--')) {
      throw new Error('--dir must be provided exactly once with a directory path.');
    }
    directory = args[index + 1];
    hasExplicitDirectory = true;
    index += 1;
  }
  if (remaining.some((argument) => argument.startsWith('--'))) {
    throw new Error(`Unknown option: ${remaining.find((argument) => argument.startsWith('--'))}`);
  }
  return { directory, args: remaining };
}

function manifestPath(directory) {
  return join(resolve(directory), 'session.json');
}

function load(directory) {
  const path = manifestPath(directory);
  if (!existsSync(path)) throw new Error(`No acceptance session at ${path}. Run init first.`);
  let data;
  try {
    data = JSON.parse(readFileSync(path, 'utf8'));
  } catch (error) {
    throw new Error(`Acceptance session is not valid JSON: ${error.message}`);
  }
  return { path, data };
}

function save(path, data) {
  data.updatedAt = new Date().toISOString();
  const temporaryPath = `${path}.tmp`;
  writeFileSync(temporaryPath, `${JSON.stringify(data, null, 2)}\n`, 'utf8');
  renameSync(temporaryPath, path);
}

function initialMetadata() {
  return Object.fromEntries(
    Object.entries(METADATA).map(([field, definition]) => [
      field,
      typeof definition.defaultValue === 'function'
        ? definition.defaultValue()
        : definition.defaultValue,
    ]),
  );
}

function init(directory) {
  const path = manifestPath(directory);
  if (existsSync(path)) throw new Error(`Acceptance session already exists at ${path}.`);
  mkdirSync(dirname(path), { recursive: true });
  const now = new Date().toISOString();
  const data = {
    schemaVersion: SCHEMA_VERSION,
    createdAt: now,
    updatedAt: now,
    metadata: initialMetadata(),
    scenarios: Object.fromEntries(
      SCENARIOS.map(([id, title, priority, minEvidence]) => [
        id,
        {
          title,
          priority,
          minEvidence,
          outcome: 'not_run',
          note: '',
          evidence: [],
          recordedAt: null,
        },
      ]),
    ),
  };
  save(path, data);
  process.stdout.write(`Created ${path}\n`);
}

function metadata(directory, field, value) {
  const { path, data } = load(directory);
  if (!Object.hasOwn(METADATA, field)) throw new Error(`Unknown metadata field: ${field}`);
  if (typeof value !== 'string') throw new Error(`metadata.${field} requires a value.`);
  data.metadata ??= {};
  data.metadata[field] = value.trim();
  save(path, data);
  process.stdout.write(`Recorded metadata.${field}\n`);
}

function pathIsInside(parent, child) {
  const pathFromParent = relative(parent, child);
  return (
    pathFromParent !== '' &&
    pathFromParent !== '..' &&
    !pathFromParent.startsWith(`..${sep}`) &&
    !isAbsolute(pathFromParent)
  );
}

function hashFile(path) {
  const descriptor = openSync(path, 'r');
  const hash = createHash('sha256');
  const buffer = Buffer.allocUnsafe(1024 * 1024);
  try {
    let bytesRead;
    do {
      bytesRead = readSync(descriptor, buffer, 0, buffer.length, null);
      if (bytesRead > 0) hash.update(buffer.subarray(0, bytesRead));
    } while (bytesRead > 0);
    return hash.digest('hex');
  } finally {
    closeSync(descriptor);
  }
}

function validateEvidencePath(base, manifest, item) {
  const absolute = resolve(item);
  if (!existsSync(absolute)) throw new Error(`Evidence does not exist: ${item}`);
  if (lstatSync(absolute).isSymbolicLink())
    throw new Error(`Evidence must not be a symlink: ${item}`);
  const realBase = realpathSync(base);
  const realEvidence = realpathSync(absolute);
  if (!pathIsInside(realBase, realEvidence)) {
    throw new Error(`Evidence must be a file inside ${base}: ${item}`);
  }
  if (realEvidence === realpathSync(manifest)) throw new Error('session.json cannot be evidence.');
  const file = statSync(realEvidence);
  if (!file.isFile() || file.size === 0)
    throw new Error(`Evidence must be a non-empty file: ${item}`);
  if (TEXT_EVIDENCE_EXTENSIONS.has(extname(realEvidence).toLowerCase())) {
    if (file.size > 2 * 1024 * 1024) {
      throw new Error(`Text evidence must be a sanitized excerpt no larger than 2 MiB: ${item}`);
    }
    const text = readFileSync(realEvidence, 'utf8');
    for (const [label, pattern] of SENSITIVE_TEXT_PATTERNS) {
      if (pattern.test(text)) throw new Error(`Evidence appears to contain ${label}: ${item}`);
    }
  }
  return {
    path: relative(base, realEvidence),
    bytes: file.size,
    sha256: hashFile(realEvidence),
  };
}

function record(directory, scenarioId, outcome, note, evidence) {
  const { path, data } = load(directory);
  const scenario = data.scenarios?.[scenarioId];
  if (!scenario) throw new Error(`Unknown scenario: ${scenarioId}`);
  if (!VALID_OUTCOMES.has(outcome) || outcome === 'not_run') {
    throw new Error('Outcome must be pass, fail, or blocked.');
  }
  if (note.trim().length < 16)
    throw new Error('The scenario note must contain at least 16 characters.');
  const base = dirname(path);
  const normalizedEvidence = evidence.map((item) => validateEvidencePath(base, path, item));
  if (
    new Set(normalizedEvidence.map((item) => item.path)).size !== normalizedEvidence.length ||
    new Set(normalizedEvidence.map((item) => item.sha256)).size !== normalizedEvidence.length
  ) {
    throw new Error(`${scenarioId} evidence files must be distinct.`);
  }
  if (outcome === 'pass' && normalizedEvidence.length < scenario.minEvidence) {
    throw new Error(`${scenarioId} pass requires at least ${scenario.minEvidence} evidence files.`);
  }
  scenario.outcome = outcome;
  scenario.note = note.trim();
  scenario.evidence = normalizedEvidence;
  scenario.recordedAt = new Date().toISOString();
  data.metadata.evidenceRedactionAttestation = '';
  save(path, data);
  process.stdout.write(`Recorded ${scenarioId}: ${outcome}\n`);
}

function validateSchema(data, problems) {
  if (data.schemaVersion !== SCHEMA_VERSION) {
    problems.push(`schemaVersion must be ${SCHEMA_VERSION}; initialize a new session`);
  }
  if (!validIsoDate(String(data.createdAt ?? ''))) problems.push('createdAt is invalid');
  if (!validIsoDate(String(data.updatedAt ?? ''))) problems.push('updatedAt is invalid');
  if (!data.metadata || typeof data.metadata !== 'object' || Array.isArray(data.metadata)) {
    problems.push('metadata object is missing');
  }
  if (!data.scenarios || typeof data.scenarios !== 'object' || Array.isArray(data.scenarios)) {
    problems.push('scenarios object is missing');
  }
}

function check(directory) {
  const { path, data } = load(directory);
  const problems = [];
  validateSchema(data, problems);

  const actualMetadata = data.metadata && typeof data.metadata === 'object' ? data.metadata : {};
  for (const [field, definition] of Object.entries(METADATA)) {
    const value = actualMetadata[field];
    if (typeof value !== 'string' || !definition.validate(value)) {
      problems.push(`metadata.${field} is missing or invalid`);
    }
  }
  for (const field of Object.keys(actualMetadata)) {
    if (!Object.hasOwn(METADATA, field)) problems.push(`unknown metadata field: ${field}`);
  }

  const actualScenarios =
    data.scenarios && typeof data.scenarios === 'object' ? data.scenarios : {};
  const evidenceOwners = new Map();
  for (const [id, title, priority, minEvidence] of SCENARIOS) {
    const scenario = actualScenarios[id];
    if (!scenario || typeof scenario !== 'object') {
      problems.push(`${id} is missing`);
      continue;
    }
    if (
      scenario.title !== title ||
      scenario.priority !== priority ||
      scenario.minEvidence !== minEvidence
    ) {
      problems.push(`${id} definition does not match schema`);
    }
    if (scenario.outcome !== 'pass') problems.push(`${id} is ${scenario.outcome ?? 'invalid'}`);
    if (typeof scenario.note !== 'string' || scenario.note.trim().length < 16) {
      problems.push(`${id} has no meaningful note`);
    }
    if (!validIsoDate(String(scenario.recordedAt ?? '')))
      problems.push(`${id} recordedAt is invalid`);
    else {
      const recordedAt = Date.parse(scenario.recordedAt);
      if (recordedAt < Date.parse(data.createdAt) || recordedAt > Date.now() + 5 * 60_000) {
        problems.push(`${id} recordedAt is outside this session`);
      }
    }
    if (!Array.isArray(scenario.evidence) || scenario.evidence.length < minEvidence) {
      problems.push(`${id} requires at least ${minEvidence} evidence files`);
      continue;
    }
    const scenarioEvidence = new Set();
    for (const item of scenario.evidence) {
      if (
        !item ||
        typeof item !== 'object' ||
        Array.isArray(item) ||
        typeof item.path !== 'string' ||
        typeof item.bytes !== 'number' ||
        !validSha256(String(item.sha256 ?? '')) ||
        Object.keys(item).some((field) => !['path', 'bytes', 'sha256'].includes(field))
      ) {
        problems.push(`${id} has invalid evidence metadata`);
        continue;
      }
      if (isAbsolute(item.path))
        problems.push(`${id} evidence paths must be relative: ${item.path}`);
      try {
        const actual = validateEvidencePath(dirname(path), path, resolve(dirname(path), item.path));
        if (actual.bytes !== item.bytes || actual.sha256 !== item.sha256) {
          problems.push(`${id} evidence changed after recording: ${item.path}`);
        }
        if (scenarioEvidence.has(actual.path) || scenarioEvidence.has(actual.sha256)) {
          problems.push(`${id} contains duplicate evidence: ${item.path}`);
          continue;
        }
        scenarioEvidence.add(actual.path);
        scenarioEvidence.add(actual.sha256);
        const previousOwner = evidenceOwners.get(actual.sha256);
        if (previousOwner && previousOwner !== id) {
          problems.push(`${id} reuses evidence already assigned to ${previousOwner}: ${item.path}`);
        } else {
          evidenceOwners.set(actual.sha256, id);
        }
      } catch (error) {
        problems.push(`${id} evidence is invalid: ${error.message}`);
      }
    }
  }
  for (const id of Object.keys(actualScenarios)) {
    if (!SCENARIOS.some(([expectedId]) => expectedId === id))
      problems.push(`unknown scenario: ${id}`);
  }

  if (problems.length > 0) {
    process.stderr.write(
      `Live beta gate BLOCKED (${problems.length} issue${problems.length === 1 ? '' : 's'}):\n`,
    );
    for (const problem of problems) process.stderr.write(`- ${problem}\n`);
    process.exitCode = 1;
    return;
  }
  process.stdout.write(`Live beta gate PASS: ${path}\n`);
}

try {
  const [command, ...rawArgs] = process.argv.slice(2);
  if (!command || command === 'help' || command === '--help' || command === '-h') usage();
  const { directory, args } = parseDirectory(rawArgs);
  if (command === 'init') {
    if (args.length !== 0) usage(2);
    init(directory);
  } else if (command === 'check') {
    if (args.length !== 0) usage(2);
    check(directory);
  } else if (command === 'metadata') {
    if (args.length !== 2) usage(2);
    metadata(directory, args[0], args[1]);
  } else if (command === 'record') {
    if (args.length < 4) usage(2);
    const [scenarioId, outcome, note, ...evidence] = args;
    record(directory, scenarioId, outcome, note, evidence);
  } else {
    usage(2);
  }
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 2;
}
