#!/usr/bin/env node

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import process from "node:process";

const SCENARIOS = [
  ["R01", "Returning identity and restart", "P0"],
  ["R02", "Password confirmation, keyboard, and lock warning", "P0"],
  ["R03", "Browser handoff and install fallback", "P0"],
  ["R04", "HTTPS and harbor invite normalization", "P0"],
  ["R05", "Contact request lifecycle and notification", "P0"],
  ["R06", "Authorized contact wall and privacy denial", "P0"],
  ["R07", "Reactive post, contact, and message refresh", "P0"],
  ["R08", "Media transfer states and retry", "P0"],
  ["R09", "Offline feed catch-up and bounded media prefetch", "P0"],
  ["R10", "Foreground, background, and missed notifications", "P0"],
  ["R11A", "Windows to macOS voice calls in both directions", "P0"],
  ["R11B", "Windows to macOS video calls in both directions", "P0"],
  ["R11C", "Three-profile group call", "P0"],
  ["R12", "Safe link cards and consent-gated embeds", "P0"],
  ["R13", "Composer, feed filters, onboarding, and bug tracking", "P0"],
  ["R14", "Pointer, keyboard, focus, and reduced-motion interaction", "P0"],
  ["R15", "Verified names replace keys on normal surfaces", "P0"],
];

const VALID_OUTCOMES = new Set(["not_run", "pass", "fail", "blocked"]);
const DEFAULT_DIR = "artifacts/live-beta-acceptance";

function usage(exitCode = 0) {
  const output = exitCode === 0 ? process.stdout : process.stderr;
  output.write(`Usage:
  pnpm acceptance:live-beta init [directory]
  pnpm acceptance:live-beta record [directory] <scenario> <pass|fail|blocked> <note> [evidence ...]
  pnpm acceptance:live-beta metadata [directory] <field> <value>
  pnpm acceptance:live-beta check [directory]

Metadata fields: commit, version, relayArtifact, relayNamespace,
windowsVersion, windowsArchitecture, macosVersion, macosArchitecture,
macosPackage, thirdProfilePlatform, operator
`);
  process.exit(exitCode);
}

function manifestPath(directory) {
  return join(resolve(directory), "session.json");
}

function load(directory) {
  const path = manifestPath(directory);
  if (!existsSync(path)) {
    throw new Error(`No acceptance session at ${path}. Run init first.`);
  }
  return { path, data: JSON.parse(readFileSync(path, "utf8")) };
}

function save(path, data) {
  data.updatedAt = new Date().toISOString();
  writeFileSync(path, `${JSON.stringify(data, null, 2)}\n`, "utf8");
}

function init(directory) {
  const path = manifestPath(directory);
  if (existsSync(path)) {
    throw new Error(`Acceptance session already exists at ${path}.`);
  }
  mkdirSync(dirname(path), { recursive: true });
  const now = new Date().toISOString();
  const data = {
    schemaVersion: 1,
    createdAt: now,
    updatedAt: now,
    metadata: {
      commit: "",
      version: "",
      relayArtifact: "",
      relayNamespace: "harbor.social",
      windowsVersion: "",
      windowsArchitecture: "",
      macosVersion: "",
      macosArchitecture: "",
      macosPackage: "",
      thirdProfilePlatform: "",
      operator: "",
    },
    scenarios: Object.fromEntries(
      SCENARIOS.map(([id, title, priority]) => [
        id,
        { title, priority, outcome: "not_run", note: "", evidence: [], recordedAt: null },
      ]),
    ),
  };
  save(path, data);
  process.stdout.write(`Created ${path}\n`);
}

function metadata(directory, field, value) {
  const { path, data } = load(directory);
  if (!Object.hasOwn(data.metadata, field)) {
    throw new Error(`Unknown metadata field: ${field}`);
  }
  data.metadata[field] = value.trim();
  save(path, data);
  process.stdout.write(`Recorded metadata.${field}\n`);
}

function record(directory, scenarioId, outcome, note, evidence) {
  const { path, data } = load(directory);
  const scenario = data.scenarios[scenarioId];
  if (!scenario) throw new Error(`Unknown scenario: ${scenarioId}`);
  if (!VALID_OUTCOMES.has(outcome) || outcome === "not_run") {
    throw new Error(`Outcome must be pass, fail, or blocked.`);
  }
  const base = dirname(path);
  const normalizedEvidence = evidence.map((item) => {
    const absolute = resolve(item);
    if (!existsSync(absolute)) throw new Error(`Evidence does not exist: ${item}`);
    return relative(base, absolute) || ".";
  });
  scenario.outcome = outcome;
  scenario.note = note.trim();
  scenario.evidence = normalizedEvidence;
  scenario.recordedAt = new Date().toISOString();
  save(path, data);
  process.stdout.write(`Recorded ${scenarioId}: ${outcome}\n`);
}

function check(directory) {
  const { path, data } = load(directory);
  const problems = [];
  for (const [field, value] of Object.entries(data.metadata)) {
    if (!String(value).trim()) problems.push(`metadata.${field} is missing`);
  }
  for (const [id, scenario] of Object.entries(data.scenarios)) {
    if (scenario.outcome !== "pass") problems.push(`${id} is ${scenario.outcome}`);
    if (!scenario.note?.trim()) problems.push(`${id} has no note`);
    if (!Array.isArray(scenario.evidence) || scenario.evidence.length === 0) {
      problems.push(`${id} has no evidence`);
    } else {
      for (const item of scenario.evidence) {
        if (!existsSync(resolve(dirname(path), item))) problems.push(`${id} evidence is missing: ${item}`);
      }
    }
  }
  if (problems.length > 0) {
    process.stderr.write(`Live beta gate BLOCKED (${problems.length} issue${problems.length === 1 ? "" : "s"}):\n`);
    for (const problem of problems) process.stderr.write(`- ${problem}\n`);
    process.exitCode = 1;
    return;
  }
  process.stdout.write(`Live beta gate PASS: ${path}\n`);
}

try {
  const [command, ...args] = process.argv.slice(2);
  if (!command || command === "help" || command === "--help" || command === "-h") usage();
  if (command === "init") init(args[0] ?? DEFAULT_DIR);
  else if (command === "check") check(args[0] ?? DEFAULT_DIR);
  else if (command === "metadata") {
    if (args.length < 2) usage(2);
    const hasExplicitDirectory = args.length >= 3;
    metadata(hasExplicitDirectory ? args[0] : DEFAULT_DIR, args.at(-2), args.at(-1));
  } else if (command === "record") {
    if (args.length < 4) usage(2);
    const firstIsScenario = Object.hasOwn(Object.fromEntries(SCENARIOS.map(([id]) => [id, true])), args[0]);
    const directory = firstIsScenario ? DEFAULT_DIR : args.shift();
    const [scenarioId, outcome, note, ...evidence] = args;
    record(directory, scenarioId, outcome, note, evidence);
  } else usage(2);
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 2;
}
