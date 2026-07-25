import assert from 'node:assert/strict';
import { existsSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import {
  createRealAdapter,
  redactSensitiveText,
  runCallHarness,
  validateHarnessConfig,
} from './cross-host-call-harness.mjs';

function config() {
  return {
    schemaVersion: 1,
    commandTimeoutMs: 500,
    convergenceTimeoutMs: 500,
    callTimeoutConvergenceMs: 500,
    pollIntervalMs: 25,
    cleanupTimeoutMs: 500,
    roles: { caller: 'alpha', callee: 'bravo', groupPeer: 'charlie' },
    endpoints: [
      {
        id: 'alpha',
        kind: 'local-wsl',
        profile: 'harness-alpha',
        displayName: 'Harness Alpha',
        dataDir: '/tmp/harbor-call-harness-alpha',
        controlPort: 24101,
        harborPath: '/opt/harbor/Harbor',
        harborctlPath: '/opt/harbor/harborctl',
        controlToken: { env: 'HARBOR_HARNESS_ALPHA_TOKEN' },
        identityPassphrase: { env: 'HARBOR_HARNESS_ALPHA_PASSWORD' },
      },
      {
        id: 'bravo',
        kind: 'windows-powershell',
        profile: 'harness-bravo',
        displayName: 'Harness Bravo',
        dataDir: 'C:\\Temp\\harbor-call-harness-bravo',
        controlPort: 24102,
        harborPath: 'C:\\Program Files\\Harbor\\Harbor.exe',
        harborctlPath: 'C:\\Program Files\\Harbor\\harborctl.exe',
        controlToken: { env: 'HARBOR_HARNESS_BRAVO_TOKEN' },
        identityPassphrase: { env: 'HARBOR_HARNESS_BRAVO_PASSWORD' },
      },
      {
        id: 'charlie',
        kind: 'remote-linux-ssh',
        profile: 'harness-charlie',
        displayName: 'Harness Charlie',
        dataDir: '/tmp/harbor-call-harness-charlie',
        controlPort: 24103,
        harborPath: '/opt/harbor/Harbor',
        harborctlPath: '/opt/harbor/harborctl',
        controlToken: { file: '/run/secrets/harbor-harness-token' },
        identityPassphrase: { file: '/run/secrets/harbor-harness-password' },
        sshTarget: 'qa@example.test',
        dialAddress: '/dns4/example.test/tcp/{tcpPort}/p2p/{peerId}',
        runtimeEnvironment: {
          WAYLAND_DISPLAY: 'wayland-0',
          XDG_RUNTIME_DIR: '/run/user/1000',
          WEBKIT_EXEC_PATH: '/opt/harbor/runtime/libexec/webkit2gtk-4.1',
        },
      },
    ],
  };
}

class FakeWorld {
  constructor(failure = null) {
    this.adapters = new Map();
    this.contacts = new Set();
    this.failure = failure;
    this.commands = [];
  }

  pair(left, right) {
    return [left, right].sort().join('|');
  }

  byPeerId(peerId) {
    return [...this.adapters.values()].find((adapter) => adapter.peerId === peerId);
  }
}

class FakeAdapter {
  constructor(spec, world) {
    this.spec = spec;
    this.world = world;
    this.peerId = `fake-profile-identifier-${spec.id}`;
    this.identity = null;
    this.profileEventsReady = false;
    this.profileRegistrationPending = false;
    this.networkRunning = false;
    this.call = { state: 'idle', terminalReason: null, peerId: null };
    this.group = { state: 'idle', participants: [] };
    this.timeoutPolls = 0;
    this.launched = false;
    this.cleaned = false;
    world.adapters.set(spec.id, this);
  }

  async launch() {
    this.launched = true;
  }

  response(result = {}) {
    return { ok: true, result, error: null };
  }

  async control(args) {
    this.world.commands.push({
      endpoint: this.spec.id,
      command: args[0],
      action: args[1] ?? null,
      arguments: args.slice(1),
    });
    if (this.world.failure?.(this, args)) {
      return {
        ok: false,
        result: null,
        error:
          'token=canary-control-token password=canary-password sdp=v=0\\r\\na=ice-pwd:secret candidate:1 1 udp 1 10.0.0.1 9 signature=[1] nonce=canary',
      };
    }
    switch (args[0]) {
      case 'status':
        return this.response({
          identity: this.identity,
          identityUnlocked: Boolean(this.identity),
          networkRunning: this.networkRunning,
        });
      case 'identity-create':
        this.identity = { peerId: this.peerId, status: 'unlocked' };
        this.profileEventsReady = false;
        this.profileRegistrationPending = false;
        return this.response(this.identity);
      case 'network-start':
        if (!this.profileEventsReady) {
          return { ok: false, result: null, error: 'profile listeners are not ready' };
        }
        this.networkRunning = true;
        return this.response();
      case 'network-stop':
        this.networkRunning = false;
        for (const adapter of this.world.adapters.values()) {
          const participant = adapter.group.participants.find(
            (item) => item.peerId === this.peerId,
          );
          if (participant) {
            participant.state = 'failed';
            adapter.group.state = adapter.group.participants.some(
              (item) => item.state === 'connected',
            )
              ? 'degraded'
              : 'failed';
          }
        }
        return this.response();
      case 'network-connect':
        return this.response();
      case 'network-addresses':
        return this.response([
          `/ip4/203.0.113.10/tcp/4001/p2p/12D3KooWRelay/p2p-circuit/p2p/${this.peerId}`,
          `/ip4/0.0.0.0/tcp/31${this.spec.controlPort}/p2p/${this.peerId}`,
        ]);
      case 'network-peers':
        return this.response(
          [...this.world.adapters.values()]
            .filter((adapter) => adapter !== this)
            .map((adapter) => ({
              peerId: adapter.peerId,
              isConnected: this.networkRunning && adapter.networkRunning,
            })),
        );
      case 'contact-request': {
        const receiver = this.world.byPeerId(args[1]);
        receiver.pendingContact = this.peerId;
        return this.response({ requestId: `request-${this.spec.id}-${receiver.spec.id}` });
      }
      case 'contact-accept':
        if (this.pendingContact !== args[1]) {
          return { ok: false, result: null, error: 'No pending contact request' };
        }
        this.world.contacts.add(this.world.pair(this.peerId, args[1]));
        this.pendingContact = null;
        return this.response({ status: 'accepted' });
      case 'contact-status': {
        const ready = this.world.contacts.has(this.world.pair(this.peerId, args[1]));
        return this.response({
          isContact: ready,
          issuedCallGrant: ready,
          receivedCallGrant: ready,
        });
      }
      case 'shutdown':
        return this.response({ shuttingDown: true });
      case 'frontend':
        return this.frontend(args[1], args[2]);
      default:
        return { ok: false, result: null, error: 'unsupported fake command' };
    }
  }

  frontend(action, rawPayload) {
    const payload = rawPayload ? JSON.parse(rawPayload) : {};
    if (action === 'identity.refresh') {
      this.profileRegistrationPending = Boolean(this.identity);
      return this.response(this.identity ? { status: 'unlocked' } : { status: 'locked' });
    }
    if (action === 'state.snapshot') {
      if (this.profileRegistrationPending) {
        this.profileRegistrationPending = false;
        this.profileEventsReady = true;
      }
      if (this.call.state === 'ringing') {
        this.timeoutPolls += 1;
        if (this.timeoutPolls >= 2) {
          const peer = this.world.byPeerId(this.call.peerId);
          this.call = { state: 'ended', terminalReason: 'timeout', peerId: peer?.peerId ?? null };
          if (peer?.call.state === 'incoming') {
            peer.call = { state: 'ended', terminalReason: 'timeout', peerId: this.peerId };
          }
        }
      }
      return this.response({
        identity: this.identity ? { status: 'unlocked' } : { status: 'locked' },
        call: { ...this.call },
        group: {
          ...this.group,
          participants: this.group.participants.map((participant) => ({ ...participant })),
        },
        error: null,
        profileEventsReady: this.profileEventsReady,
      });
    }
    if (action === 'call.start') {
      const callee = this.world.byPeerId(payload.peerId);
      this.timeoutPolls = 0;
      this.call = { state: 'ringing', terminalReason: null, peerId: callee.peerId };
      callee.call = { state: 'incoming', terminalReason: null, peerId: this.peerId };
      return this.response({ ...this.call });
    }
    if (action === 'call.accept') {
      if (this.call.state !== 'incoming') {
        return { ok: false, result: null, error: 'No current incoming call' };
      }
      const caller = this.world.byPeerId(this.call.peerId);
      this.call.state = 'connected';
      caller.call.state = 'connected';
      return this.response({ ...this.call });
    }
    if (action === 'call.hangup') {
      const peer = this.world.byPeerId(this.call.peerId);
      this.call.state = 'ended';
      this.call.terminalReason ??= 'normal';
      if (peer) {
        peer.call.state = 'ended';
        peer.call.terminalReason ??= 'normal';
      }
      return this.response({ ...this.call });
    }
    if (action === 'group.start') {
      this.group = {
        state: 'ringing',
        participants: payload.peerIds.map((peerId) => ({ peerId, state: 'ringing' })),
      };
      for (const peerId of payload.peerIds) {
        const peer = this.world.byPeerId(peerId);
        peer.group = {
          state: 'ringing',
          participants: [{ peerId: this.peerId, state: 'ringing' }],
        };
      }
      return this.response(this.group);
    }
    if (action === 'group.accept') {
      const creator = [...this.world.adapters.values()].find((adapter) =>
        adapter.group.participants.some((participant) => participant.peerId === this.peerId),
      );
      this.group.state = 'connected';
      this.group.participants[0].state = 'connected';
      creator.group.participants.find((participant) => participant.peerId === this.peerId).state =
        'connected';
      creator.group.state = 'connected';
      return this.response(this.group);
    }
    if (action === 'group.leave') {
      for (const adapter of this.world.adapters.values()) {
        adapter.group = { state: 'idle', participants: [] };
      }
      return this.response(this.group);
    }
    return { ok: false, result: null, error: 'unsupported fake frontend action' };
  }

  async cleanup() {
    this.cleaned = true;
    this.networkRunning = false;
  }
}

test('validates all three host adapters and rejects inline secrets or dev artifacts', () => {
  const valid = validateHarnessConfig(config());
  assert.deepEqual(
    valid.endpoints.map((endpoint) => endpoint.kind),
    ['local-wsl', 'windows-powershell', 'remote-linux-ssh'],
  );

  const inline = config();
  inline.endpoints[0].controlToken = { value: 'never-allowed' };
  assert.throws(() => validateHarnessConfig(inline), /cannot contain inline secret material/);

  const sourceBuild = config();
  sourceBuild.endpoints[0].harborPath = '/repo/src-tauri/target/debug/harbor';
  assert.throws(() => validateHarnessConfig(sourceBuild), /installed or packaged artifact/);

  const remoteEnvironment = config();
  remoteEnvironment.endpoints[2].controlToken = { env: 'REMOTE_TOKEN' };
  assert.throws(() => validateHarnessConfig(remoteEnvironment), /host-local token file for SSH/);

  assert.deepEqual(valid.endpoints[2].runtimeEnvironment, {
    WAYLAND_DISPLAY: 'wayland-0',
    XDG_RUNTIME_DIR: '/run/user/1000',
    WEBKIT_EXEC_PATH: '/opt/harbor/runtime/libexec/webkit2gtk-4.1',
  });
  const unsafeRuntimeEnvironment = config();
  unsafeRuntimeEnvironment.endpoints[2].runtimeEnvironment.HARBOR_CONTROL_TOKEN = 'inline-secret';
  assert.throws(
    () => validateHarnessConfig(unsafeRuntimeEnvironment),
    /not an allowed runtime variable/,
  );

  const unsafeEphemeralPath = config();
  unsafeEphemeralPath.endpoints[0].controlToken = {
    file: '/tmp/operator-token',
    ownership: 'harness-ephemeral',
  };
  assert.throws(
    () => validateHarnessConfig(unsafeEphemeralPath),
    /name starts with harbor-call-harness-/,
  );

  const unknownOwnership = config();
  unknownOwnership.endpoints[0].controlToken = {
    file: '/tmp/harbor-call-harness-token',
    ownership: 'shared',
  };
  assert.throws(() => validateHarnessConfig(unknownOwnership), /operator or harness-ephemeral/);
});

test('real adapter removes only explicitly harness-owned ephemeral secret files', async () => {
  const root = mkdtempSync(join(tmpdir(), 'harbor-call-harness-secrets-'));
  const dataDir = join(root, 'harbor-call-harness-profile');
  const ephemeralToken = join(root, 'harbor-call-harness-token');
  const operatorPassword = join(root, 'operator-password');
  writeFileSync(ephemeralToken, 'disposable-token\n', { mode: 0o600 });
  writeFileSync(operatorPassword, 'operator-password\n', { mode: 0o600 });
  const input = config();
  input.endpoints[0] = {
    ...input.endpoints[0],
    dataDir,
    controlToken: { file: ephemeralToken, ownership: 'harness-ephemeral' },
    identityPassphrase: { file: operatorPassword },
  };
  const validated = validateHarnessConfig(input);
  const adapter = createRealAdapter(validated.endpoints[0], validated);
  try {
    await adapter.cleanup();
    assert.equal(existsSync(ephemeralToken), false);
    assert.equal(existsSync(operatorPassword), true);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('fake adapters exercise every scenario with bounded convergence and complete teardown', async () => {
  const world = new FakeWorld();
  const result = await runCallHarness(config(), {
    adapterFactory: (spec) => new FakeAdapter(spec, world),
    writeEvidence: false,
  });

  assert.equal(result.outcome, 'pass');
  assert.deepEqual(
    result.scenarios.map(({ id, outcome }) => [id, outcome]),
    [
      ['provision', 'pass'],
      ['voice', 'pass'],
      ['reverse-voice', 'pass'],
      ['video', 'pass'],
      ['arm64-voice', 'pass'],
      ['group-partial-failure', 'pass'],
      ['call-after-group', 'pass'],
      ['timeout-late-answer', 'pass'],
    ],
  );
  assert.equal(
    [...world.adapters.values()].every((adapter) => adapter.cleaned),
    true,
  );
  assert.equal(
    result.cleanup.every((item) => item.outcome === 'complete'),
    true,
  );
  assert.equal(world.commands.filter(({ command }) => command === 'network-connect').length, 2);
  assert.equal(
    world.commands
      .filter(({ command }) => command === 'network-connect')
      .every(
        ({ arguments: [address] }) => address.includes('/tcp/31') && !address.includes('/tcp/4001'),
      ),
    true,
  );
  assert.equal(world.commands.filter(({ command }) => command === 'network-addresses').length, 1);
  assert.equal(
    world.commands.filter(
      ({ command, action }) => command === 'frontend' && action === 'identity.refresh',
    ).length,
    3,
  );
  for (const endpoint of ['alpha', 'bravo', 'charlie']) {
    const endpointCommands = world.commands.filter((command) => command.endpoint === endpoint);
    const refreshIndex = endpointCommands.findIndex(
      ({ command, action }) => command === 'frontend' && action === 'identity.refresh',
    );
    const readySnapshotIndex = endpointCommands.findIndex(
      ({ command, action }, index) =>
        index > refreshIndex && command === 'frontend' && action === 'state.snapshot',
    );
    const networkStartIndex = endpointCommands.findIndex(
      ({ command }) => command === 'network-start',
    );
    assert.ok(refreshIndex >= 0, `${endpoint} did not refresh its frontend identity`);
    assert.ok(
      readySnapshotIndex > refreshIndex,
      `${endpoint} did not observe profile listener readiness`,
    );
    assert.ok(
      networkStartIndex > readySnapshotIndex,
      `${endpoint} started networking before profile listeners were ready`,
    );
  }
});

test('a secret-bearing host failure is redacted and still cleans every endpoint', async () => {
  const world = new FakeWorld(
    (adapter, args) =>
      adapter.spec.id === 'bravo' && args[0] === 'frontend' && args[1] === 'call.start',
  );
  const result = await runCallHarness(config(), {
    adapterFactory: (spec) => new FakeAdapter(spec, world),
    writeEvidence: false,
  });
  const serialized = JSON.stringify(result);

  assert.equal(result.outcome, 'fail');
  assert.equal(
    [...world.adapters.values()].every((adapter) => adapter.cleaned),
    true,
  );
  for (const forbidden of [
    'canary-control-token',
    'canary-password',
    'ice-pwd',
    'candidate:1',
    'signature=[1]',
    'nonce=canary',
  ]) {
    assert.equal(serialized.includes(forbidden), false, `leaked ${forbidden}`);
  }
});

test('privacy redaction removes identity keys, SDP, ICE, signatures, nonces, and tokens', () => {
  const unsafe = `-----BEGIN PRIVATE KEY-----\nprivate-material\n-----END PRIVATE KEY-----
token=control-secret password=identity-secret
v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na=ice-pwd:ice-secret
candidate:1 1 udp 2122260223 192.168.1.2 54400 typ host
fingerprint=private-fingerprint signature=[91,92] nonce=control-token
12D3KooWQmLongRawProfileIdentifier123456789`;
  const redacted = redactSensitiveText(unsafe, ['control-secret', 'identity-secret']);
  for (const forbidden of [
    'private-material',
    'control-secret',
    'identity-secret',
    'm=audio',
    'ice-secret',
    '192.168.1.2',
    'private-fingerprint',
    '[91,92]',
    'control-token',
    '12D3KooWQmLongRawProfileIdentifier123456789',
  ]) {
    assert.equal(redacted.includes(forbidden), false, `leaked ${forbidden}`);
  }
});

test('CLI check is non-mutating and run refuses to launch without the explicit opt-in', () => {
  const root = mkdtempSync(join(tmpdir(), 'harbor-call-harness-test-'));
  const path = join(root, 'config.json');
  writeFileSync(path, JSON.stringify(config()), 'utf8');
  const script = resolve('scripts/cross-host-call-harness.mjs');
  try {
    const checked = spawnSync(process.execPath, [script, 'check', '--config', path], {
      encoding: 'utf8',
    });
    assert.equal(checked.status, 0, checked.stderr);
    assert.match(checked.stdout, /"ok":true/);
    assert.doesNotMatch(checked.stdout, /TOKEN|PASSWORD|secret/i);

    const refused = spawnSync(process.execPath, [script, 'run', '--config', path], {
      encoding: 'utf8',
    });
    assert.notEqual(refused.status, 0);
    assert.match(refused.stderr, /execute-real-hosts|invalid command-line/i);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
