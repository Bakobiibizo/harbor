import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import toast from 'react-hot-toast';
import { useIdentityStore, useNetworkStore, useContactsStore, useSettingsStore } from '../stores';
import { contactsService } from '../services/contacts';
import * as networkService from '../services/network';
import {
  NetworkIcon,
  UsersIcon,
  UserIcon,
  SearchIcon,
  PlusIcon,
  CheckIcon,
  XIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  TrashIcon,
} from '../components/icons';
import { RELAY_CLOUDFORMATION_TEMPLATE } from '../constants/cloudformation-template';

// Adjectives and animals for generating human-friendly peer names
const ADJECTIVES = [
  'Swift',
  'Brave',
  'Calm',
  'Clever',
  'Eager',
  'Gentle',
  'Happy',
  'Jolly',
  'Kind',
  'Lively',
  'Merry',
  'Noble',
  'Proud',
  'Quick',
  'Quiet',
  'Sleek',
  'Smart',
  'Sunny',
  'Warm',
  'Wise',
  'Bold',
  'Bright',
  'Cool',
  'Crisp',
  'Dapper',
  'Fresh',
  'Grand',
  'Lucky',
  'Neat',
  'Sharp',
  'Vivid',
  'Witty',
];

const ANIMALS = [
  'Falcon',
  'Wolf',
  'Bear',
  'Eagle',
  'Hawk',
  'Lion',
  'Tiger',
  'Otter',
  'Fox',
  'Deer',
  'Owl',
  'Raven',
  'Swan',
  'Crane',
  'Heron',
  'Panda',
  'Koala',
  'Dolphin',
  'Whale',
  'Seal',
  'Lynx',
  'Badger',
  'Hare',
  'Finch',
  'Robin',
  'Sparrow',
  'Jay',
  'Wren',
  'Lark',
  'Dove',
  'Elk',
  'Moose',
];

function getPeerFriendlyName(peerId: string): string {
  let hash = 0;
  for (let characterIndex = 0; characterIndex < peerId.length; characterIndex++) {
    const charCode = peerId.charCodeAt(characterIndex);
    hash = (hash << 5) - hash + charCode;
    hash = hash & hash;
  }
  const adjIndex = Math.abs(hash) % ADJECTIVES.length;
  const animalIndex = Math.abs(hash >> 8) % ANIMALS.length;
  return `${ADJECTIVES[adjIndex]} ${ANIMALS[animalIndex]}`;
}

function getPeerColor(peerId: string): string {
  const colors = [
    'linear-gradient(135deg, hsl(220 91% 54%), hsl(262 83% 58%))',
    'linear-gradient(135deg, hsl(262 83% 58%), hsl(330 81% 60%))',
    'linear-gradient(135deg, hsl(152 69% 40%), hsl(180 70% 45%))',
    'linear-gradient(135deg, hsl(36 90% 55%), hsl(15 80% 55%))',
    'linear-gradient(135deg, hsl(200 80% 50%), hsl(220 91% 54%))',
    'linear-gradient(135deg, hsl(340 75% 55%), hsl(10 80% 60%))',
    'linear-gradient(135deg, hsl(280 70% 50%), hsl(320 75% 55%))',
    'linear-gradient(135deg, hsl(170 65% 45%), hsl(200 70% 50%))',
  ];
  let hash = 0;
  for (let characterIndex = 0; characterIndex < peerId.length; characterIndex++) {
    hash = peerId.charCodeAt(characterIndex) + ((hash << 5) - hash);
  }
  return colors[Math.abs(hash) % colors.length];
}

// Inline toggle component
function Toggle({ enabled, onChange }: { enabled: boolean; onChange: (value: boolean) => void }) {
  return (
    <button
      onClick={() => onChange(!enabled)}
      className="w-12 h-6 rounded-full relative transition-colors duration-200"
      style={{
        background: enabled ? 'hsl(var(--harbor-primary))' : 'hsl(var(--harbor-surface-2))',
      }}
    >
      <div
        className="w-5 h-5 rounded-full absolute top-0.5 transition-all duration-200"
        style={{
          background: 'white',
          left: enabled ? 'calc(100% - 22px)' : '2px',
        }}
      />
    </button>
  );
}

// Copy button helper
function CopyButton({ text, label }: { text: string; label?: string }) {
  return (
    <button
      className="p-1.5 rounded hover:bg-white/10 flex-shrink-0"
      onClick={() => {
        navigator.clipboard.writeText(text);
        toast.success(label || 'Copied!');
      }}
      title="Copy"
    >
      <svg
        className="w-4 h-4"
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
        style={{ color: 'hsl(var(--harbor-text-secondary))' }}
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={2}
          d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
        />
      </svg>
    </button>
  );
}

export function NetworkPage() {
  const { state } = useIdentityStore();
  const {
    isRunning,
    connectedPeers,
    stats,
    shareableAddresses,
    relayStatus,
    error,
    isLoading,
    startNetwork,
    stopNetwork,
    refreshPeers,
    refreshStats,
    refreshAddresses,
    refreshShareableAddresses,
    checkStatus,
    connectToPeer,
    connectToRelay,
    connectToPublicRelays,
  } = useNetworkStore();

  const { contacts, refreshContacts } = useContactsStore();
  const { autoStartNetwork, localDiscovery, setAutoStartNetwork, setLocalDiscovery } =
    useSettingsStore();

  // Local state
  const [searchQuery, setSearchQuery] = useState('');
  const [activeTab, setActiveTab] = useState<'discovered' | 'connected' | 'contacts'>('connected');
  const [relayInput, setRelayInput] = useState('');
  const [isConnectingRelay, setIsConnectingRelay] = useState(false);
  const [peerAddress, setPeerAddress] = useState('');
  const [isConnectingPeer, setIsConnectingPeer] = useState(false);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [natDetectionTimedOut, setNatDetectionTimedOut] = useState(false);
  const [shareableContactString, setShareableContactString] = useState<string | null>(null);
  const relayTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Check network status on mount and set up refresh interval
  useEffect(() => {
    checkStatus();

    const interval = setInterval(() => {
      if (isRunning) {
        refreshPeers();
        refreshStats();
        refreshAddresses();
        refreshShareableAddresses();
      }
    }, 5000);

    return () => clearInterval(interval);
  }, [
    isRunning,
    checkStatus,
    refreshPeers,
    refreshStats,
    refreshAddresses,
    refreshShareableAddresses,
  ]);

  // NAT status "Detecting..." timeout (30s)
  useEffect(() => {
    if (stats.natStatus !== 'unknown' || !isRunning) {
      setNatDetectionTimedOut(false);
      return;
    }

    const timeout = setTimeout(() => {
      setNatDetectionTimedOut(true);
    }, 30_000);

    return () => clearTimeout(timeout);
  }, [isRunning, stats.natStatus]);

  // Fetch shareable contact string when relay is connected
  useEffect(() => {
    if (relayStatus === 'connected' && isRunning) {
      networkService
        .getShareableContactString()
        .then(setShareableContactString)
        .catch((err) => {
          console.warn('Could not get shareable contact string:', err);
          setShareableContactString(null);
        });
    } else {
      setShareableContactString(null);
    }
  }, [relayStatus, isRunning]);

  // Fix 3: Relay "Connecting..." spinner timeout (30s)
  useEffect(() => {
    if (relayTimeoutRef.current) {
      clearTimeout(relayTimeoutRef.current);
      relayTimeoutRef.current = null;
    }

    if (relayStatus === 'connecting') {
      relayTimeoutRef.current = setTimeout(() => {
        const currentStatus = useNetworkStore.getState().relayStatus;
        if (currentStatus === 'connecting') {
          useNetworkStore.getState().setRelayStatus('disconnected');
          toast.error('Relay connection timed out. The relay may be unreachable.');
        }
      }, 30_000);
    }

    return () => {
      if (relayTimeoutRef.current) {
        clearTimeout(relayTimeoutRef.current);
        relayTimeoutRef.current = null;
      }
    };
  }, [relayStatus]);

  // Fix 4: NAT status "Detecting..." timeout (30s)
  useEffect(() => {
    if (stats.natStatus !== 'unknown' || !isRunning) {
      setNatDetectionTimedOut(false);
      return;
    }

    const timeout = setTimeout(() => {
      setNatDetectionTimedOut(true);
    }, 30_000);

    return () => clearTimeout(timeout);
  }, [isRunning, stats.natStatus]);

  // Handlers
  const handleConnectToRelay = async () => {
    if (!relayInput.trim()) {
      toast.error('Please enter a relay address');
      return;
    }
    if (!relayInput.includes('/p2p/')) {
      toast.error('Relay address must include peer ID (/p2p/...)');
      return;
    }
    setIsConnectingRelay(true);
    try {
      await connectToRelay(relayInput.trim());
      toast.success('Connecting to relay...');
      setRelayInput('');
    } catch (err) {
      toast.error(`Failed to connect to relay: ${err}`);
    } finally {
      setIsConnectingRelay(false);
    }
  };

  const handleConnectToPublicRelays = async () => {
    setIsConnectingRelay(true);
    try {
      await connectToPublicRelays();
      toast.success('Connecting to Harbor relay...');
    } catch (err) {
      toast.error(`Failed to connect to public relays: ${err}`);
    } finally {
      setIsConnectingRelay(false);
    }
  };

  const handleConnectToPeer = async () => {
    const input = peerAddress.trim();
    if (!input) {
      toast.error('Please enter an address');
      return;
    }

    setIsConnectingPeer(true);
    try {
      // Check if it's a harbor:// contact string (new simplified flow)
      if (input.startsWith('harbor://')) {
        await networkService.addContactFromString(input);
        toast.success('Contact added successfully!');
        refreshContacts();
        setPeerAddress('');
      } else if (input.includes('/p2p/')) {
        // Legacy multiaddr format - just connect (requires identity exchange)
        await connectToPeer(input);
        toast.success('Connection initiated!');
        setPeerAddress('');
      } else {
        toast.error('Invalid address format. Use a harbor:// link or multiaddress with /p2p/');
      }
    } catch (err) {
      toast.error(`Failed: ${err}`);
    } finally {
      setIsConnectingPeer(false);
    }
  };

  const formatBytes = (bytes: number) => {
    if (!bytes || isNaN(bytes)) return '0 B';
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  const formatUptime = (seconds: number) => {
    if (!seconds || isNaN(seconds)) return '0s';
    const hours = Math.floor(seconds / 3600);
    const mins = Math.floor((seconds % 3600) / 60);
    const secs = Math.floor(seconds % 60);
    if (hours > 0) return `${hours}h ${mins}m`;
    if (mins > 0) return `${mins}m ${secs}s`;
    return `${secs}s`;
  };

  // Filter peers by search (also checks contact display names)
  const filteredPeers = connectedPeers.filter((peer) => {
    const query = searchQuery.toLowerCase();
    if (!query) return true;
    const friendlyName = getPeerFriendlyName(peer.peerId).toLowerCase();
    const contactName = contacts.find((contact) => contact.peerId === peer.peerId)?.displayName?.toLowerCase() ?? '';
    return (
      friendlyName.includes(query) ||
      contactName.includes(query) ||
      peer.peerId.toLowerCase().includes(query) ||
      peer.addresses.some((addr) => addr.toLowerCase().includes(query))
    );
  });

  const discoveredPeers = filteredPeers.filter((peer) => !peer.isConnected);
  const connectedPeersList = filteredPeers.filter((peer) => peer.isConnected);
  const filteredContacts = contacts.filter((contact) => {
    const query = searchQuery.toLowerCase();
    if (!query) return true;
    return (
      contact.displayName.toLowerCase().includes(query) ||
      contact.peerId.toLowerCase().includes(query)
    );
  });

  // Get the circuit address (the one shareable address for remote peers)
  const circuitAddress =
    shareableAddresses.length > 0
      ? shareableAddresses[0]
      : stats.relayAddresses.length > 0
        ? stats.relayAddresses[0]
        : null;

  return (
    <div className="h-full flex flex-col" style={{ background: 'hsl(var(--harbor-bg-primary))' }}>
      {/* Section A: Header + Control */}
      <header
        className="px-6 py-4 border-b flex-shrink-0"
        style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
      >
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-xl font-bold" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
              Network
            </h1>
            <p className="text-sm mt-0.5" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
              Manage connections, relays, and peers
            </p>
          </div>
          <button
            onClick={isRunning ? stopNetwork : startNetwork}
            disabled={isLoading || state.status !== 'unlocked'}
            className="flex items-center gap-2 px-4 py-2 rounded-xl font-medium transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed"
            style={{
              background: isRunning
                ? 'hsl(var(--harbor-error) / 0.1)'
                : 'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
              color: isRunning ? 'hsl(var(--harbor-error))' : 'white',
              border: isRunning ? '1px solid hsl(var(--harbor-error) / 0.2)' : 'none',
              boxShadow: isRunning ? 'none' : '0 4px 12px hsl(var(--harbor-primary) / 0.3)',
            }}
          >
            {isLoading ? (
              <div
                className="w-4 h-4 border-2 rounded-full animate-spin"
                style={{
                  borderColor: isRunning
                    ? 'hsl(var(--harbor-error) / 0.3)'
                    : 'rgba(255,255,255,0.3)',
                  borderTopColor: isRunning ? 'hsl(var(--harbor-error))' : 'white',
                }}
              />
            ) : isRunning ? (
              <XIcon className="w-4 h-4" />
            ) : (
              <NetworkIcon className="w-4 h-4" />
            )}
            {isLoading ? '...' : isRunning ? 'Stop Network' : 'Start Network'}
          </button>
        </div>
      </header>

      <div className="flex-1 overflow-y-auto p-6">
        <div className="max-w-4xl mx-auto space-y-6">
          {/* Error banner */}
          {error && (
            <div
              className="p-4 rounded-xl flex items-center gap-3"
              style={{
                background: 'hsl(var(--harbor-error) / 0.1)',
                border: '1px solid hsl(var(--harbor-error) / 0.2)',
              }}
            >
              <XIcon
                className="w-5 h-5 flex-shrink-0"
                style={{ color: 'hsl(var(--harbor-error))' }}
              />
              <p style={{ color: 'hsl(var(--harbor-error))' }}>{error}</p>
            </div>
          )}

          {/* Stats row (when running) */}
          {isRunning && (
            <div className="grid grid-cols-2 md:grid-cols-5 gap-3">
              <div
                className="p-3 rounded-xl"
                style={{
                  background: 'hsl(var(--harbor-bg-elevated))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                }}
              >
                <p className="text-xs mb-1" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                  Peers
                </p>
                <p
                  className="text-xl font-bold"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  {stats.connectedPeers}
                </p>
              </div>
              <div
                className="p-3 rounded-xl"
                style={{
                  background: 'hsl(var(--harbor-bg-elevated))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                }}
              >
                <p className="text-xs mb-1" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                  Uptime
                </p>
                <p
                  className="text-xl font-bold"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  {formatUptime(stats.uptimeSeconds)}
                </p>
              </div>
              <div
                className="p-3 rounded-xl"
                style={{
                  background: 'hsl(var(--harbor-bg-elevated))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                }}
              >
                <p className="text-xs mb-1" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                  Received
                </p>
                <p
                  className="text-xl font-bold"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  {formatBytes(stats.totalBytesIn)}
                </p>
              </div>
              <div
                className="p-3 rounded-xl"
                style={{
                  background: 'hsl(var(--harbor-bg-elevated))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                }}
              >
                <p className="text-xs mb-1" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                  Sent
                </p>
                <p
                  className="text-xl font-bold"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  {formatBytes(stats.totalBytesOut)}
                </p>
              </div>
              <div
                className="p-3 rounded-xl"
                style={{
                  background:
                    stats.natStatus === 'public'
                      ? 'hsl(var(--harbor-success) / 0.1)'
                      : stats.natStatus === 'private'
                        ? 'hsl(var(--harbor-warning) / 0.1)'
                        : 'hsl(var(--harbor-bg-elevated))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                }}
              >
                <p className="text-xs mb-1" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                  NAT Status
                </p>
                <div className="flex items-center gap-2">
                  <div
                    className="w-2 h-2 rounded-full"
                    style={{
                      background:
                        stats.natStatus === 'public'
                          ? 'hsl(var(--harbor-success))'
                          : stats.natStatus === 'private'
                            ? 'hsl(var(--harbor-warning))'
                            : 'hsl(var(--harbor-text-tertiary))',
                    }}
                  />
                  <p
                    className="text-sm font-semibold capitalize"
                    style={{
                      color:
                        stats.natStatus === 'public'
                          ? 'hsl(var(--harbor-success))'
                          : stats.natStatus === 'private'
                            ? 'hsl(var(--harbor-warning))'
                            : 'hsl(var(--harbor-text-primary))',
                    }}
                  >
                    {stats.natStatus === 'unknown'
                      ? (natDetectionTimedOut ? 'Unable to detect' : 'Detecting...')
                      : stats.natStatus === 'public' ? 'Public' : stats.natStatus === 'private' ? 'Relayed' : 'Behind NAT'}
                  </p>
                </div>
              </div>
            </div>
          )}

          {/* Section B: Relay Connection */}
          <div
            className="rounded-2xl p-6"
            style={{
              background: 'hsl(var(--harbor-bg-elevated))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
            }}
          >
            <h3
              className="text-sm font-medium mb-4"
              style={{ color: 'hsl(var(--harbor-text-secondary))' }}
            >
              Harbor Relay
            </h3>

            {relayStatus === 'connected' && stats.relayAddresses.length > 0 ? (
              // Connected state: show green relay status
              <div>
                <div
                  className="flex items-center gap-3 p-3 rounded-lg mb-3"
                  style={{
                    background: 'hsl(var(--harbor-success) / 0.1)',
                    border: '1px solid hsl(var(--harbor-success) / 0.2)',
                  }}
                >
                  <span
                    className="w-2.5 h-2.5 rounded-full animate-pulse flex-shrink-0"
                    style={{ background: 'hsl(var(--harbor-success))' }}
                  />
                  <div className="flex-1 min-w-0">
                    <p
                      className="text-sm font-medium"
                      style={{ color: 'hsl(var(--harbor-success))' }}
                    >
                      Connected to Harbor Relay
                    </p>
                    <code
                      className="text-xs break-all font-mono block mt-1"
                      style={{ color: 'hsl(var(--harbor-success) / 0.8)' }}
                    >
                      {stats.relayAddresses[0]}
                    </code>
                  </div>
                  <CopyButton text={stats.relayAddresses[0]} label="Relay address copied!" />
                </div>
                <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                  Peers anywhere on the internet can reach you through this relay.
                </p>
              </div>
            ) : (
              // Disconnected/connecting state: show connect options
              <div className="space-y-3">
                {relayStatus === 'connecting' && (
                  <div
                    className="flex items-center gap-3 p-3 rounded-lg"
                    style={{
                      background: 'hsl(var(--harbor-warning) / 0.1)',
                      border: '1px solid hsl(var(--harbor-warning) / 0.2)',
                    }}
                  >
                    <div
                      className="w-4 h-4 border-2 rounded-full animate-spin flex-shrink-0"
                      style={{
                        borderColor: 'hsl(var(--harbor-warning) / 0.3)',
                        borderTopColor: 'hsl(var(--harbor-warning))',
                      }}
                    />
                    <p className="text-sm" style={{ color: 'hsl(var(--harbor-warning))' }}>
                      Connecting to relay...
                    </p>
                  </div>
                )}

                {/* Quick connect to Harbor relay */}
                <button
                  onClick={handleConnectToPublicRelays}
                  disabled={!isRunning || isConnectingRelay}
                  className="w-full flex items-center justify-center gap-2 px-4 py-3 rounded-lg text-sm font-medium transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                  style={{
                    background:
                      'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                    color: 'white',
                    boxShadow: '0 2px 8px hsl(var(--harbor-primary) / 0.3)',
                  }}
                >
                  <NetworkIcon className="w-4 h-4" />
                  Connect to Harbor Relay
                </button>

                {/* Custom relay input */}
                <div className="flex gap-2">
                  <input
                    type="text"
                    value={relayInput}
                    onChange={(event) => setRelayInput(event.target.value)}
                    placeholder="/ip4/YOUR_IP/tcp/4001/p2p/PEER_ID"
                    className="flex-1 px-3 py-2 rounded-lg text-sm font-mono"
                    style={{
                      background: 'hsl(var(--harbor-surface-1))',
                      border: '1px solid hsl(var(--harbor-border-subtle))',
                      color: 'hsl(var(--harbor-text-primary))',
                    }}
                    disabled={!isRunning || isConnectingRelay}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter') handleConnectToRelay();
                    }}
                  />
                  <button
                    onClick={handleConnectToRelay}
                    disabled={!isRunning || isConnectingRelay || !relayInput.trim()}
                    className="px-4 py-2 rounded-lg font-medium text-sm transition-colors disabled:opacity-50"
                    style={{
                      background: 'hsl(var(--harbor-surface-1))',
                      color: 'hsl(var(--harbor-text-primary))',
                      border: '1px solid hsl(var(--harbor-border-subtle))',
                    }}
                  >
                    Connect
                  </button>
                </div>

                {!isRunning && (
                  <p className="text-xs" style={{ color: 'hsl(var(--harbor-warning))' }}>
                    Start the network first to connect to a relay.
                  </p>
                )}
              </div>
            )}

            {/* Settings toggles */}
            <div
              className="mt-4 pt-4 border-t space-y-3"
              style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
            >
              <div className="flex items-center justify-between">
                <div>
                  <p
                    className="text-sm font-medium"
                    style={{ color: 'hsl(var(--harbor-text-primary))' }}
                  >
                    Auto-connect on startup
                  </p>
                  <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                    Automatically connect to the network when app starts
                  </p>
                </div>
                <Toggle enabled={autoStartNetwork} onChange={setAutoStartNetwork} />
              </div>
              <div className="flex items-center justify-between">
                <div>
                  <p
                    className="text-sm font-medium"
                    style={{ color: 'hsl(var(--harbor-text-primary))' }}
                  >
                    Local network discovery
                  </p>
                  <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                    Find other Harbor users on your local network
                  </p>
                </div>
                <Toggle enabled={localDiscovery} onChange={setLocalDiscovery} />
              </div>
            </div>
          </div>

          {/* Your Circuit Address (only when running) */}
          {isRunning && (
            <div
              className="rounded-2xl p-6"
              style={{
                background: 'hsl(var(--harbor-bg-elevated))',
                border: '1px solid hsl(var(--harbor-border-subtle))',
              }}
            >
              <h3
                className="text-sm font-medium mb-3"
                style={{ color: 'hsl(var(--harbor-text-secondary))' }}
              >
                Your Address
              </h3>

              {shareableContactString ? (
                <div>
                  <div
                    className="flex items-center gap-2 p-3 rounded-lg mb-2"
                    style={{
                      background: 'hsl(var(--harbor-success) / 0.1)',
                      border: '1px solid hsl(var(--harbor-success) / 0.2)',
                    }}
                  >
                    <span
                      className="w-2 h-2 rounded-full animate-pulse flex-shrink-0"
                      style={{ background: 'hsl(var(--harbor-success))' }}
                    />
                    <code
                      className="text-xs flex-1 break-all font-mono"
                      style={{ color: 'hsl(var(--harbor-success))' }}
                    >
                      {shareableContactString}
                    </code>
                    <CopyButton
                      text={shareableContactString}
                      label="Contact link copied! Share this with your contact."
                    />
                  </div>
                  <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                    Copy this link and send it to the person you want to connect with. They can
                    paste it below to instantly add you.
                  </p>
                </div>
              ) : circuitAddress ? (
                <div>
                  <div
                    className="flex items-center gap-2 p-3 rounded-lg mb-2"
                    style={{
                      background: 'hsl(var(--harbor-warning) / 0.1)',
                      border: '1px solid hsl(var(--harbor-warning) / 0.2)',
                    }}
                  >
                    <span
                      className="w-2 h-2 rounded-full flex-shrink-0"
                      style={{ background: 'hsl(var(--harbor-warning))' }}
                    />
                    <code
                      className="text-xs flex-1 break-all font-mono"
                      style={{ color: 'hsl(var(--harbor-warning))' }}
                    >
                      {circuitAddress}
                    </code>
                    <CopyButton text={circuitAddress} label="Address copied (legacy format)" />
                  </div>
                  <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                    Generating shareable contact link...
                  </p>
                </div>
              ) : (
                <div
                  className="p-3 rounded-lg"
                  style={{
                    background: 'hsl(var(--harbor-surface-1))',
                    border: '1px solid hsl(var(--harbor-border-subtle))',
                  }}
                >
                  <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                    {relayStatus === 'connecting'
                      ? 'Waiting for relay connection to generate your address...'
                      : 'Connect to the Harbor relay to get your shareable address.'}
                  </p>
                </div>
              )}
            </div>
          )}

          {/* Connect to a Peer */}
          {isRunning && (
            <div
              className="rounded-2xl p-6"
              style={{
                background: 'hsl(var(--harbor-bg-elevated))',
                border: '1px solid hsl(var(--harbor-border-subtle))',
              }}
            >
              <h3
                className="text-sm font-medium mb-3"
                style={{ color: 'hsl(var(--harbor-text-secondary))' }}
              >
                Add a Contact
              </h3>

              <div className="flex gap-2 mb-3">
                <input
                  type="text"
                  value={peerAddress}
                  onChange={(event) => setPeerAddress(event.target.value)}
                  placeholder="Paste a harbor:// link here"
                  className="flex-1 px-3 py-2 rounded-lg text-sm font-mono"
                  style={{
                    background: 'hsl(var(--harbor-surface-1))',
                    border: '1px solid hsl(var(--harbor-border-subtle))',
                    color: 'hsl(var(--harbor-text-primary))',
                  }}
                  disabled={isConnectingPeer}
                  onKeyDown={(event) => {
                    if (event.key === 'Enter') handleConnectToPeer();
                  }}
                />
                <button
                  onClick={handleConnectToPeer}
                  disabled={isConnectingPeer || !peerAddress.trim()}
                  className="px-4 py-2 rounded-lg font-medium text-sm transition-colors disabled:opacity-50"
                  style={{
                    background:
                      'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                    color: 'white',
                  }}
                >
                  {isConnectingPeer ? 'Adding...' : 'Add Contact'}
                </button>
              </div>

              <div
                className="p-3 rounded-lg"
                style={{ background: 'hsl(var(--harbor-surface-1))' }}
              >
                <p
                  className="text-xs font-medium mb-2"
                  style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                >
                  How to connect with someone:
                </p>
                <ol
                  className="text-xs space-y-1 list-decimal list-inside"
                  style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                >
                  <li>Both users start their network (auto-connects to relay)</li>
                  <li>
                    Copy your{' '}
                    <code
                      className="px-1 py-0.5 rounded"
                      style={{ background: 'hsl(var(--harbor-surface-2))' }}
                    >
                      harbor://
                    </code>{' '}
                    link from above and send it to your contact
                  </li>
                  <li>Ask your contact to send you their link</li>
                  <li>
                    Paste their link here and click Add - they'll be instantly added as a contact!
                  </li>
                </ol>
              </div>
            </div>
          )}

          {/* Section D: Peers (consolidated tabs) */}
          <div
            className="rounded-2xl overflow-hidden"
            style={{
              background: 'hsl(var(--harbor-bg-elevated))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
            }}
          >
            {/* Tabs and Search */}
            <div
              className="p-4 border-b flex items-center gap-4"
              style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
            >
              <div
                className="flex rounded-lg p-1"
                style={{ background: 'hsl(var(--harbor-surface-1))' }}
              >
                <button
                  onClick={() => setActiveTab('discovered')}
                  className="px-3 py-1.5 rounded-md text-sm font-medium transition-all duration-200"
                  style={{
                    background:
                      activeTab === 'discovered' ? 'hsl(var(--harbor-bg-elevated))' : 'transparent',
                    color:
                      activeTab === 'discovered'
                        ? 'hsl(var(--harbor-text-primary))'
                        : 'hsl(var(--harbor-text-tertiary))',
                    boxShadow: activeTab === 'discovered' ? 'var(--shadow-sm)' : 'none',
                  }}
                >
                  <SearchIcon className="w-4 h-4 inline mr-1" />
                  Discovered
                  {discoveredPeers.length > 0 && (
                    <span
                      className="ml-1.5 px-1.5 py-0.5 rounded-full text-xs"
                      style={{
                        background: 'hsl(var(--harbor-primary) / 0.15)',
                        color: 'hsl(var(--harbor-primary))',
                      }}
                    >
                      {discoveredPeers.length}
                    </span>
                  )}
                </button>
                <button
                  onClick={() => setActiveTab('connected')}
                  className="px-3 py-1.5 rounded-md text-sm font-medium transition-all duration-200"
                  style={{
                    background:
                      activeTab === 'connected' ? 'hsl(var(--harbor-bg-elevated))' : 'transparent',
                    color:
                      activeTab === 'connected'
                        ? 'hsl(var(--harbor-text-primary))'
                        : 'hsl(var(--harbor-text-tertiary))',
                    boxShadow: activeTab === 'connected' ? 'var(--shadow-sm)' : 'none',
                  }}
                >
                  <UsersIcon className="w-4 h-4 inline mr-1" />
                  Connected
                  {connectedPeersList.length > 0 && (
                    <span
                      className="ml-1.5 px-1.5 py-0.5 rounded-full text-xs"
                      style={{
                        background: 'hsl(var(--harbor-success) / 0.15)',
                        color: 'hsl(var(--harbor-success))',
                      }}
                    >
                      {connectedPeersList.length}
                    </span>
                  )}
                </button>
                <button
                  onClick={() => setActiveTab('contacts')}
                  className="px-3 py-1.5 rounded-md text-sm font-medium transition-all duration-200"
                  style={{
                    background:
                      activeTab === 'contacts' ? 'hsl(var(--harbor-bg-elevated))' : 'transparent',
                    color:
                      activeTab === 'contacts'
                        ? 'hsl(var(--harbor-text-primary))'
                        : 'hsl(var(--harbor-text-tertiary))',
                    boxShadow: activeTab === 'contacts' ? 'var(--shadow-sm)' : 'none',
                  }}
                >
                  <UserIcon className="w-4 h-4 inline mr-1" />
                  Contacts
                  {contacts.length > 0 && (
                    <span
                      className="ml-1.5 px-1.5 py-0.5 rounded-full text-xs"
                      style={{
                        background: 'hsl(var(--harbor-primary) / 0.15)',
                        color: 'hsl(var(--harbor-primary))',
                      }}
                    >
                      {contacts.length}
                    </span>
                  )}
                </button>
              </div>

              <div className="flex-1 relative">
                <SearchIcon
                  className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4"
                  style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                />
                <input
                  type="text"
                  placeholder="Search..."
                  value={searchQuery}
                  onChange={(event) => setSearchQuery(event.target.value)}
                  className="w-full pl-10 pr-4 py-2 rounded-lg text-sm"
                  style={{
                    background: 'hsl(var(--harbor-surface-1))',
                    border: '1px solid hsl(var(--harbor-border-subtle))',
                    color: 'hsl(var(--harbor-text-primary))',
                  }}
                />
              </div>
            </div>

            {/* Peer list content */}
            <div className="p-4">
              {!isRunning ? (
                <div className="text-center py-12">
                  <div
                    className="w-16 h-16 rounded-full flex items-center justify-center mx-auto mb-4"
                    style={{ background: 'hsl(var(--harbor-surface-1))' }}
                  >
                    <NetworkIcon
                      className="w-8 h-8"
                      style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                    />
                  </div>
                  <p
                    className="font-medium mb-1"
                    style={{ color: 'hsl(var(--harbor-text-primary))' }}
                  >
                    Network is offline
                  </p>
                  <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                    Start the network to discover peers
                  </p>
                </div>
              ) : activeTab === 'discovered' ? (
                // Discovered peers tab
                discoveredPeers.length === 0 ? (
                  <div className="text-center py-12">
                    <div
                      className="w-16 h-16 rounded-full flex items-center justify-center mx-auto mb-4"
                      style={{ background: 'hsl(var(--harbor-surface-1))' }}
                    >
                      <SearchIcon
                        className="w-8 h-8"
                        style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                      />
                    </div>
                    <p
                      className="font-medium mb-1"
                      style={{ color: 'hsl(var(--harbor-text-primary))' }}
                    >
                      {searchQuery ? 'No peers found' : 'No discovered peers'}
                    </p>
                    <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                      {searchQuery
                        ? 'Try a different search term'
                        : 'Peers will appear here as they are discovered via mDNS or DHT'}
                    </p>
                  </div>
                ) : (
                  <div className="space-y-2">
                    {discoveredPeers.map((peer) => {
                      const knownContact = contacts.find((contact) => contact.peerId === peer.peerId);
                      return (
                        <PeerRow
                          key={peer.peerId}
                          peerId={peer.peerId}
                          displayName={knownContact?.displayName}
                          actionLabel="Connect"
                          actionStyle="primary"
                          onAction={async () => {
                            if (peer.addresses.length > 0) {
                              try {
                                await connectToPeer(peer.addresses[0]);
                                toast.success('Connecting...');
                              } catch (err) {
                                toast.error(`Failed: ${err}`);
                              }
                            } else {
                              toast.error('No address available for this peer');
                            }
                          }}
                        />
                      );
                    })}
                  </div>
                )
              ) : activeTab === 'connected' ? (
                // Connected peers tab
                connectedPeersList.length === 0 ? (
                  <div className="text-center py-12">
                    <div
                      className="w-16 h-16 rounded-full flex items-center justify-center mx-auto mb-4"
                      style={{ background: 'hsl(var(--harbor-surface-1))' }}
                    >
                      <UsersIcon
                        className="w-8 h-8"
                        style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                      />
                    </div>
                    <p
                      className="font-medium mb-1"
                      style={{ color: 'hsl(var(--harbor-text-primary))' }}
                    >
                      {searchQuery ? 'No peers found' : 'No connected peers'}
                    </p>
                    <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                      {searchQuery
                        ? 'Try a different search term'
                        : 'Connected peers will appear here'}
                    </p>
                  </div>
                ) : (
                  <div className="space-y-2">
                    {connectedPeersList.map((peer) => {
                      const knownContact = contacts.find((contact) => contact.peerId === peer.peerId);
                      const displayName = knownContact?.displayName;
                      return (
                        <PeerRow
                          key={peer.peerId}
                          peerId={peer.peerId}
                          displayName={displayName}
                          isConnected
                          actionLabel={knownContact ? 'Message' : 'Add Contact'}
                          actionStyle="success"
                          onAction={async () => {
                            try {
                              await contactsService.requestPeerIdentity(peer.peerId);
                              toast.success(`Requesting identity from ${displayName ?? getPeerFriendlyName(peer.peerId)}...`);
                            } catch (err) {
                              toast.error(`Failed to add contact: ${err}`);
                            }
                          }}
                        />
                      );
                    })}
                  </div>
                )
              ) : // Contacts tab
              filteredContacts.length === 0 ? (
                <div className="text-center py-12">
                  <div
                    className="w-16 h-16 rounded-full flex items-center justify-center mx-auto mb-4"
                    style={{ background: 'hsl(var(--harbor-surface-1))' }}
                  >
                    <UserIcon
                      className="w-8 h-8"
                      style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                    />
                  </div>
                  <p
                    className="font-medium mb-1"
                    style={{ color: 'hsl(var(--harbor-text-primary))' }}
                  >
                    {searchQuery ? 'No contacts found' : 'No contacts yet'}
                  </p>
                  <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                    {searchQuery
                      ? 'Try a different search term'
                      : 'Add contacts by connecting to peers or using their Peer ID'}
                  </p>
                </div>
              ) : (
                <div className="space-y-2">
                  {filteredContacts.map((contact) => (
                    <div
                      key={contact.peerId}
                      className="flex items-center gap-4 p-3 rounded-xl"
                      style={{ background: 'hsl(var(--harbor-surface-1))' }}
                    >
                      <div
                        className="w-10 h-10 rounded-full flex items-center justify-center text-sm font-semibold text-white flex-shrink-0"
                        style={{ background: getPeerColor(contact.peerId) }}
                      >
                        {contact.displayName
                          .split(' ')
                          .map((word) => word[0])
                          .join('')
                          .toUpperCase()
                          .slice(0, 2)}
                      </div>
                      <div className="flex-1 min-w-0">
                        <p
                          className="font-medium text-sm"
                          style={{ color: 'hsl(var(--harbor-text-primary))' }}
                        >
                          {contact.displayName}
                        </p>
                        <p
                          className="text-xs font-mono truncate"
                          style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                          title={contact.peerId}
                        >
                          {contact.peerId.slice(0, 12)}...{contact.peerId.slice(-6)}
                        </p>
                      </div>
                      {contact.bio && (
                        <p
                          className="text-xs truncate max-w-[200px]"
                          style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                        >
                          {contact.bio}
                        </p>
                      )}
                      <button
                        className="p-2 rounded-lg hover:bg-white/10 transition-colors flex-shrink-0"
                        style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                        title="Remove contact"
                        onClick={async () => {
                          try {
                            await contactsService.removeContact(contact.peerId);
                            await refreshContacts();
                            toast.success(`Removed ${contact.displayName} from contacts`);
                          } catch (err) {
                            toast.error(`Failed to remove contact: ${err}`);
                          }
                        }}
                      >
                        <TrashIcon className="w-4 h-4" />
                      </button>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>

          {/* Advanced (collapsible) */}
          <div
            className="rounded-2xl p-6"
            style={{
              background: 'hsl(var(--harbor-bg-elevated))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
            }}
          >
            <button
              onClick={() => setShowAdvanced(!showAdvanced)}
              className="flex items-center gap-2 text-sm font-medium transition-colors w-full"
              style={{ color: 'hsl(var(--harbor-text-secondary))' }}
            >
              {showAdvanced ? (
                <ChevronDownIcon className="w-4 h-4" />
              ) : (
                <ChevronRightIcon className="w-4 h-4" />
              )}
              Advanced
            </button>

            {showAdvanced && (
              <div className="mt-4">
                <DeployRelayContent />
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

// --- Sub-components ---

function PeerRow({
  peerId,
  displayName,
  isConnected,
  actionLabel,
  actionStyle,
  onAction,
}: {
  peerId: string;
  displayName?: string;
  isConnected?: boolean;
  actionLabel: string;
  actionStyle: 'primary' | 'success';
  onAction: () => Promise<void>;
}) {
  const friendlyName = displayName ?? getPeerFriendlyName(peerId);
  const avatarColor = getPeerColor(peerId);
  const initials = friendlyName.split(' ').map((word) => word[0]).join('').toUpperCase().slice(0, 2);

  return (
    <div
      className="flex items-center gap-4 p-3 rounded-xl transition-all duration-200"
      style={{ background: 'hsl(var(--harbor-surface-1))' }}
    >
      <div
        className="w-10 h-10 rounded-full flex items-center justify-center text-sm font-semibold text-white flex-shrink-0"
        style={{ background: avatarColor }}
      >
        {initials}
      </div>
      <div className="flex-1 min-w-0">
        <p className="font-medium text-sm" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
          {friendlyName}
        </p>
        <p
          className="text-xs font-mono truncate"
          style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
          title={peerId}
        >
          {peerId.slice(0, 12)}...{peerId.slice(-6)}
        </p>
      </div>

      {isConnected && (
        <div className="flex items-center gap-1.5">
          <div
            className="w-2 h-2 rounded-full animate-pulse"
            style={{ background: 'hsl(var(--harbor-success))' }}
          />
          <span className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
            Connected
          </span>
        </div>
      )}

      <button
        className="px-3 py-1.5 rounded-lg text-xs font-medium transition-colors"
        style={
          actionStyle === 'success'
            ? {
                background: 'hsl(var(--harbor-success) / 0.15)',
                color: 'hsl(var(--harbor-success))',
              }
            : {
                background:
                  'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                color: 'white',
              }
        }
        title={actionLabel}
        onClick={onAction}
      >
        {actionStyle === 'success' ? (
          <span className="flex items-center gap-1">
            <PlusIcon className="w-3.5 h-3.5" />
            {actionLabel}
          </span>
        ) : (
          <span className="flex items-center gap-1">
            <CheckIcon className="w-3.5 h-3.5" />
            {actionLabel}
          </span>
        )}
      </button>
    </div>
  );
}

function DeployRelayContent() {
  return (
    <div className="mt-4 space-y-4">
      <div className="flex items-start gap-3">
        <div
          className="w-10 h-10 rounded-xl flex items-center justify-center flex-shrink-0"
          style={{ background: 'hsl(var(--harbor-primary) / 0.15)' }}
        >
          <svg
            className="w-5 h-5"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
            style={{ color: 'hsl(var(--harbor-primary))' }}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M5 12h14M5 12a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v4a2 2 0 01-2 2M5 12a2 2 0 00-2 2v4a2 2 0 002 2h14a2 2 0 002-2v-4a2 2 0 00-2-2m-2-4h.01M17 16h.01"
            />
          </svg>
        </div>
        <div>
          <p className="text-sm font-medium" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
            Deploy Your Own Relay Server
          </p>
          <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
            Run your own relay on AWS for better connectivity. Free tier eligible!
          </p>
        </div>
      </div>

      {/* Step 1: Download */}
      <div className="p-3 rounded-lg" style={{ background: 'hsl(var(--harbor-surface-1))' }}>
        <div className="flex items-center gap-2 mb-2">
          <span
            className="w-5 h-5 rounded-full flex items-center justify-center text-xs font-bold"
            style={{ background: 'linear-gradient(135deg, #FF9900, #FF6600)', color: 'white' }}
          >
            1
          </span>
          <span
            className="text-xs font-medium"
            style={{ color: 'hsl(var(--harbor-text-primary))' }}
          >
            Download template
          </span>
        </div>
        <button
          onClick={async () => {
            try {
              const savedPath = await invoke<string>('save_to_downloads', {
                filename: 'harbor-relay-cloudformation.yaml',
                content: RELAY_CLOUDFORMATION_TEMPLATE,
              });
              toast.success(`Template saved to ${savedPath}`);
            } catch (error) {
              console.error('Failed to save template via Tauri:', error);
              // Fallback: copy to clipboard
              try {
                await navigator.clipboard.writeText(RELAY_CLOUDFORMATION_TEMPLATE);
                toast.success('Template copied to clipboard! Paste it into a .yaml file.');
              } catch {
                toast.error(`Save failed: ${error}`);
              }
            }
          }}
          className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium"
          style={{ background: 'linear-gradient(135deg, #FF9900, #FF6600)', color: 'white' }}
        >
          <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"
            />
          </svg>
          Download Template
        </button>
      </div>

      {/* Step 2: Open AWS */}
      <div className="p-3 rounded-lg" style={{ background: 'hsl(var(--harbor-surface-1))' }}>
        <div className="flex items-center gap-2 mb-2">
          <span
            className="w-5 h-5 rounded-full flex items-center justify-center text-xs font-bold"
            style={{ background: 'linear-gradient(135deg, #FF9900, #FF6600)', color: 'white' }}
          >
            2
          </span>
          <span
            className="text-xs font-medium"
            style={{ color: 'hsl(var(--harbor-text-primary))' }}
          >
            Upload to AWS CloudFormation
          </span>
        </div>
        <a
          href="https://console.aws.amazon.com/cloudformation/home#/stacks/create/template"
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium"
          style={{ background: 'hsl(var(--harbor-primary))', color: 'white' }}
        >
          Open AWS CloudFormation
        </a>
        <div
          className="text-xs mt-2 space-y-0.5"
          style={{ color: 'hsl(var(--harbor-text-secondary))' }}
        >
          <p>Select "Upload a template file", choose the downloaded file, click "Next"</p>
        </div>
      </div>

      {/* Step 3: Configure */}
      <div className="p-3 rounded-lg" style={{ background: 'hsl(var(--harbor-surface-1))' }}>
        <div className="flex items-center gap-2 mb-2">
          <span
            className="w-5 h-5 rounded-full flex items-center justify-center text-xs font-bold"
            style={{ background: 'linear-gradient(135deg, #FF9900, #FF6600)', color: 'white' }}
          >
            3
          </span>
          <span
            className="text-xs font-medium"
            style={{ color: 'hsl(var(--harbor-text-primary))' }}
          >
            Configure: stack name "harbor-relay", leave defaults, click Submit
          </span>
        </div>
        <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
          Check the IAM acknowledgement box on the review page. Wait 3-5 min for CREATE_COMPLETE.
        </p>
      </div>

      {/* Step 4: Get address */}
      <div
        className="p-3 rounded-lg"
        style={{
          background: 'hsl(var(--harbor-success) / 0.1)',
          border: '1px solid hsl(var(--harbor-success) / 0.2)',
        }}
      >
        <div className="flex items-center gap-2 mb-2">
          <span
            className="w-5 h-5 rounded-full flex items-center justify-center text-xs font-bold"
            style={{ background: 'hsl(var(--harbor-success))', color: 'white' }}
          >
            4
          </span>
          <span className="text-xs font-medium" style={{ color: 'hsl(var(--harbor-success))' }}>
            Get your relay address from the Outputs tab
          </span>
        </div>
        <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
          Go to your stack's "Outputs" tab, click "Step2GetYourRelayAddress", copy the value. Paste
          it in the relay input above.
        </p>
      </div>

      {/* Cost info */}
      <div
        className="p-3 rounded-lg flex items-start gap-2"
        style={{ background: 'hsl(var(--harbor-success) / 0.1)' }}
      >
        <svg
          className="w-4 h-4 flex-shrink-0 mt-0.5"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
          style={{ color: 'hsl(var(--harbor-success))' }}
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
          />
        </svg>
        <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
          <strong style={{ color: 'hsl(var(--harbor-success))' }}>Free for 12 months!</strong> AWS
          Free Tier includes 750 hours/month of t2.micro. After: ~$9-12/month.
        </p>
      </div>

      {/* Cleanup */}
      <div className="flex items-center justify-between">
        <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
          Need to delete your relay? Remove all resources to stop charges.
        </p>
        <a
          href="https://console.aws.amazon.com/cloudformation/home#/stacks?filteringStatus=active&filteringText=harbor-relay"
          target="_blank"
          rel="noopener noreferrer"
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium flex-shrink-0"
          style={{
            background: 'hsl(var(--harbor-error) / 0.1)',
            color: 'hsl(var(--harbor-error))',
          }}
        >
          Manage Stacks
        </a>
      </div>
    </div>
  );
}
