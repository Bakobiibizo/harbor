import type {
  CallIceError,
  CallIceErrorCode,
  CallIceStateSnapshot,
  IceServerConfig,
  IceServerInput,
  IceServerProtocol,
  IceServerValidationResult,
  RedactedIceServerConfig,
  TurnCredentialPersistence,
} from '../types';

const ALLOWED_ICE_PROTOCOLS: IceServerProtocol[] = ['stun', 'stuns', 'turn', 'turns'];
const TURN_PROTOCOLS = new Set<IceServerProtocol>(['turn', 'turns']);
const DEFAULT_TURN_CREDENTIAL_PERSISTENCE: TurnCredentialPersistence = 'session';

/** Split operator-entered ICE URLs from textarea/comma/space separated input. */
export function parseIceServerUrls(value: string | string[]): string[] {
  const values = Array.isArray(value) ? value : value.split(/[\s,]+/u);
  return values.map((url) => url.trim()).filter(Boolean);
}

function getIceProtocol(url: string): IceServerProtocol | null {
  const match = url.match(/^([a-z][a-z0-9+.-]*):/iu);
  if (!match) return null;
  const protocol = match[1].toLowerCase() as IceServerProtocol;
  return ALLOWED_ICE_PROTOCOLS.includes(protocol) ? protocol : null;
}

function urlLooksLikeEmbeddedCredential(url: string): boolean {
  const schemeIndex = url.indexOf(':');
  const queryIndex = url.indexOf('?');
  const credentialRegion =
    queryIndex === -1 ? url.slice(schemeIndex + 1) : url.slice(schemeIndex + 1, queryIndex);
  return credentialRegion.includes('@');
}

function containsTurnUrl(urls: string[]): boolean {
  return urls.some((url) => {
    const protocol = getIceProtocol(url);
    return protocol !== null && TURN_PROTOCOLS.has(protocol);
  });
}

function buildIceServerId(urls: string[], username?: string): string {
  const source = `${urls.join('|')}|${username ?? ''}`;
  let hash = 2166136261;
  for (let index = 0; index < source.length; index += 1) {
    hash ^= source.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return `ice-${(hash >>> 0).toString(36)}`;
}

function hasUsableTurnServer(server: IceServerConfig): boolean {
  return containsTurnUrl(server.urls) && Boolean(server.username && server.credential);
}

function hasConfiguredTurnServer(server: IceServerConfig): boolean {
  return containsTurnUrl(server.urls);
}

function normalizePersistence(value?: TurnCredentialPersistence): TurnCredentialPersistence {
  return value === 'device' ? 'device' : DEFAULT_TURN_CREDENTIAL_PERSISTENCE;
}

/** Validate and normalize an ICE server entry before it is stored or passed to WebRTC. */
export function validateIceServerInput(
  input: IceServerInput,
  existingServers: IceServerConfig[] = [],
): IceServerValidationResult {
  const urls = Array.from(new Set(parseIceServerUrls(input.urls)));

  if (urls.length === 0) {
    return { ok: false, error: 'Enter at least one STUN or TURN URL.' };
  }

  for (const url of urls) {
    if (url.length > 512) {
      return { ok: false, error: 'ICE server URLs must be 512 characters or fewer.' };
    }

    const protocol = getIceProtocol(url);
    if (!protocol) {
      return {
        ok: false,
        error: 'ICE server URLs must start with stun:, stuns:, turn:, or turns:.',
      };
    }

    const remainder = url.slice(url.indexOf(':') + 1).replace(/^\/\//u, '');
    if (!remainder || remainder.startsWith('?')) {
      return { ok: false, error: `ICE server URL is missing a host: ${url}` };
    }

    if (urlLooksLikeEmbeddedCredential(url)) {
      return {
        ok: false,
        error: 'Do not embed TURN credentials in the URL. Use the username and credential fields.',
      };
    }
  }

  const alreadyConfigured = existingServers.some((server) =>
    server.urls.some((existingUrl) => urls.includes(existingUrl)),
  );
  if (alreadyConfigured) {
    return { ok: false, error: 'One of these ICE server URLs is already configured.' };
  }

  const includesTurn = containsTurnUrl(urls);
  const username = input.username?.trim() || undefined;
  const credential = input.credential?.trim() || undefined;

  if (includesTurn && (!username || !credential)) {
    return { ok: false, error: 'TURN/TURNS servers require both username and credential.' };
  }

  if (!includesTurn && (username || credential)) {
    return { ok: false, error: 'Credentials are only valid for TURN/TURNS server entries.' };
  }

  const server: IceServerConfig = {
    id: input.id?.trim() || buildIceServerId(urls, username),
    urls,
    username,
    credential,
    credentialPersistence: includesTurn
      ? normalizePersistence(input.credentialPersistence)
      : undefined,
  };

  return { ok: true, server };
}

/** Remove session-only TURN secrets before writing settings to persistent storage. */
export function stripSessionCredentialsForPersistence(server: IceServerConfig): IceServerConfig {
  if (server.credentialPersistence !== 'session') {
    return { ...server };
  }

  return {
    id: server.id,
    urls: [...server.urls],
    username: server.username,
    credentialPersistence: server.credentialPersistence,
  };
}

/** Redact an ICE server entry for display, logs, and diagnostics. */
export function redactIceServer(server: IceServerConfig): RedactedIceServerConfig {
  const hasCredential = Boolean(server.credential);
  return {
    id: server.id,
    urls: [...server.urls],
    username: server.username,
    credentialPersistence: server.credentialPersistence,
    hasCredential,
    redactedCredential: hasCredential ? '••••••••' : undefined,
  };
}

export function redactIceServers(servers: IceServerConfig[]): RedactedIceServerConfig[] {
  return servers.map(redactIceServer);
}

/** Convert validated Harbor ICE settings into browser RTCIceServer objects. */
export function buildRtcIceServers(servers: IceServerConfig[]): RTCIceServer[] {
  return servers
    .filter((server) => !hasConfiguredTurnServer(server) || hasUsableTurnServer(server))
    .map((server) => ({
      urls: [...server.urls],
      username: server.username,
      credential: server.credential,
    }));
}

export function buildRtcConfiguration(
  servers: IceServerConfig[],
  iceTransportPolicy: RTCIceTransportPolicy = 'all',
): RTCConfiguration {
  return {
    iceServers: buildRtcIceServers(servers),
    iceTransportPolicy,
  };
}

export function describeIceFailure(
  servers: IceServerConfig[],
  iceTransportPolicy: RTCIceTransportPolicy = 'all',
): CallIceError {
  const hasConfiguredTurn = servers.some(hasConfiguredTurnServer);
  const hasUsableTurn = servers.some(hasUsableTurnServer);

  let code: CallIceErrorCode;
  let message: string;
  let recoverable = true;

  if (iceTransportPolicy === 'relay' && !hasUsableTurn) {
    code = 'relay-only-without-turn';
    message =
      'WebRTC media relay-only mode requires a configured TURN server. Harbor libp2p relays can carry signaling, but they do not relay voice media.';
  } else if (hasConfiguredTurn && !hasUsableTurn) {
    code = 'turn-credentials-missing';
    message =
      'A TURN server is configured without an available credential for this session. Re-enter the TURN credential or choose device persistence.';
  } else if (!hasUsableTurn) {
    code = 'strict-nat-no-turn';
    message =
      'ICE failed without a usable TURN server. LAN/direct calls can still work, but strict NAT pairs need an operator-provided TURN server for WebRTC media.';
  } else {
    code = 'ice-failed';
    message =
      'ICE connection failed even with the configured ICE servers. Check TURN reachability, firewall policy, and credential validity.';
    recoverable = false;
  }

  return {
    code,
    message,
    recoverable,
    details: {
      iceTransportPolicy,
      iceServers: redactIceServers(servers),
    },
  };
}

export interface CreateCallPeerConnectionOptions {
  iceServers: IceServerConfig[];
  iceTransportPolicy?: RTCIceTransportPolicy;
  peerConnectionFactory?: (configuration: RTCConfiguration) => RTCPeerConnection;
  onStateChange?: (state: CallIceStateSnapshot) => void;
  onIceCandidate?: (candidate: RTCIceCandidate) => void;
}

export interface CallPeerConnectionRuntime {
  peerConnection: RTCPeerConnection;
  configuration: RTCConfiguration;
  getState: () => CallIceStateSnapshot;
  close: () => void;
}

function snapshotFromConnection(
  peerConnection: RTCPeerConnection,
  error: CallIceError | null,
): CallIceStateSnapshot {
  return {
    iceGatheringState: peerConnection.iceGatheringState,
    iceConnectionState: peerConnection.iceConnectionState,
    connectionState: peerConnection.connectionState,
    error,
  };
}

/**
 * Create the browser WebRTC runtime for Harbor calls using operator-configured ICE servers.
 * Signaling still flows through Harbor/libp2p; TURN is only for WebRTC audio media relay.
 */
export function createCallPeerConnection(
  options: CreateCallPeerConnectionOptions,
): CallPeerConnectionRuntime {
  const iceTransportPolicy = options.iceTransportPolicy ?? 'all';
  const configuration = buildRtcConfiguration(options.iceServers, iceTransportPolicy);
  const factory =
    options.peerConnectionFactory ??
    ((rtcConfiguration: RTCConfiguration) => {
      if (typeof RTCPeerConnection === 'undefined') {
        throw new Error(
          'WebRTC is unavailable in this runtime. Voice calls require RTCPeerConnection.',
        );
      }
      return new RTCPeerConnection(rtcConfiguration);
    });

  const peerConnection = factory(configuration);
  let currentError: CallIceError | null = null;
  let currentState = snapshotFromConnection(peerConnection, currentError);

  const emitState = (error: CallIceError | null = currentError) => {
    currentError = error;
    currentState = snapshotFromConnection(peerConnection, currentError);
    options.onStateChange?.(currentState);
  };

  const handleIceGatheringStateChange = () => emitState();

  const handleConnectionStateChange = () => {
    const failed =
      peerConnection.iceConnectionState === 'failed' || peerConnection.connectionState === 'failed';
    emitState(failed ? describeIceFailure(options.iceServers, iceTransportPolicy) : null);
  };

  const handleIceCandidate = (event: RTCPeerConnectionIceEvent) => {
    if (event.candidate) {
      options.onIceCandidate?.(event.candidate);
    }
  };

  const handleIceCandidateError = (event: RTCPeerConnectionIceErrorEvent) => {
    emitState({
      code: 'ice-candidate-error',
      message: `ICE candidate gathering failed for ${event.url || 'an ICE server'}: ${event.errorText || event.errorCode}`,
      recoverable: true,
      details: {
        url: event.url,
        errorCode: event.errorCode,
        errorText: event.errorText,
        iceServers: redactIceServers(options.iceServers),
      },
    });
  };

  peerConnection.addEventListener('icegatheringstatechange', handleIceGatheringStateChange);
  peerConnection.addEventListener('iceconnectionstatechange', handleConnectionStateChange);
  peerConnection.addEventListener('connectionstatechange', handleConnectionStateChange);
  peerConnection.addEventListener('icecandidate', handleIceCandidate);
  peerConnection.addEventListener('icecandidateerror', handleIceCandidateError);

  emitState();

  return {
    peerConnection,
    configuration,
    getState: () => currentState,
    close: () => {
      peerConnection.removeEventListener('icegatheringstatechange', handleIceGatheringStateChange);
      peerConnection.removeEventListener('iceconnectionstatechange', handleConnectionStateChange);
      peerConnection.removeEventListener('connectionstatechange', handleConnectionStateChange);
      peerConnection.removeEventListener('icecandidate', handleIceCandidate);
      peerConnection.removeEventListener('icecandidateerror', handleIceCandidateError);
      peerConnection.close();
    },
  };
}
