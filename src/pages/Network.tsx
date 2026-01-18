import { useEffect, useState } from 'react';
import toast from 'react-hot-toast';
import { useIdentityStore, useNetworkStore } from '../stores';
import { contactsService } from '../services/contacts';
import {
  NetworkIcon,
  UserIcon,
  UsersIcon,
  SearchIcon,
  PlusIcon,
  CheckIcon,
  XIcon,
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

// Generate a consistent human-friendly name from a peer ID
function getPeerFriendlyName(peerId: string): string {
  // Use the peer ID to generate consistent indices
  let hash = 0;
  for (let i = 0; i < peerId.length; i++) {
    const char = peerId.charCodeAt(i);
    hash = (hash << 5) - hash + char;
    hash = hash & hash; // Convert to 32-bit integer
  }

  const adjIndex = Math.abs(hash) % ADJECTIVES.length;
  const animalIndex = Math.abs(hash >> 8) % ANIMALS.length;

  return `${ADJECTIVES[adjIndex]} ${ANIMALS[animalIndex]}`;
}

// Generate consistent avatar color from peer ID
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
  for (let i = 0; i < peerId.length; i++) {
    hash = peerId.charCodeAt(i) + ((hash << 5) - hash);
  }

  return colors[Math.abs(hash) % colors.length];
}

export function NetworkPage() {
  const { state } = useIdentityStore();
  const {
    isRunning,
    status,
    connectedPeers,
    stats,
    listeningAddresses,
    error,
    isLoading,
    startNetwork,
    stopNetwork,
    refreshPeers,
    refreshStats,
    refreshAddresses,
    checkStatus,
    connectToPeer,
    addBootstrapNode,
  } = useNetworkStore();

  const [searchQuery, setSearchQuery] = useState('');
  const [activeTab, setActiveTab] = useState<'peers' | 'contacts'>('peers');
  const [showAddContactModal, setShowAddContactModal] = useState(false);
  const [showConnectPeerModal, setShowConnectPeerModal] = useState(false);
  const [manualPeerId, setManualPeerId] = useState('');
  const [peerMultiaddr, setPeerMultiaddr] = useState('');
  const [isAddingContact, setIsAddingContact] = useState(false);
  const [isConnecting, setIsConnecting] = useState(false);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [customRelayAddress, setCustomRelayAddress] = useState('');
  const [isAddingRelay, setIsAddingRelay] = useState(false);

  const identity = state.status === 'unlocked' ? state.identity : null;

  // Check network status on mount and set up refresh interval
  useEffect(() => {
    checkStatus();

    // Refresh peers, stats, and addresses every 5 seconds when running
    const interval = setInterval(() => {
      if (isRunning) {
        refreshPeers();
        refreshStats();
        refreshAddresses();
      }
    }, 5000);

    return () => clearInterval(interval);
  }, [isRunning, checkStatus, refreshPeers, refreshStats, refreshAddresses]);

  // Handle connecting to a peer by multiaddress
  const handleConnectToPeer = async () => {
    if (!peerMultiaddr.trim()) {
      toast.error('Please enter a multiaddress');
      return;
    }

    // Basic validation
    if (!peerMultiaddr.includes('/p2p/')) {
      toast.error('Multiaddress must include peer ID (/p2p/...)');
      return;
    }

    setIsConnecting(true);
    try {
      await connectToPeer(peerMultiaddr.trim());
      toast.success('Connection initiated!');
      setShowConnectPeerModal(false);
      setPeerMultiaddr('');
    } catch (err) {
      console.error('Failed to connect:', err);
      toast.error(`Failed to connect: ${err}`);
    } finally {
      setIsConnecting(false);
    }
  };

  const formatBytes = (bytes: number) => {
    // Handle NaN, undefined, or null
    if (!bytes || isNaN(bytes)) return '0 B';
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  const formatUptime = (seconds: number) => {
    // Handle NaN, undefined, or null
    if (!seconds || isNaN(seconds)) return '0s';
    const hours = Math.floor(seconds / 3600);
    const mins = Math.floor((seconds % 3600) / 60);
    const secs = Math.floor(seconds % 60);
    if (hours > 0) return `${hours}h ${mins}m`;
    if (mins > 0) return `${mins}m ${secs}s`;
    return `${secs}s`;
  };

  const getInitials = (name: string) => {
    return name
      .split(' ')
      .map((n) => n[0])
      .join('')
      .toUpperCase()
      .slice(0, 2);
  };

  const filteredPeers = connectedPeers.filter((peer) => {
    const query = searchQuery.toLowerCase();
    const friendlyName = getPeerFriendlyName(peer.peerId).toLowerCase();
    return (
      friendlyName.includes(query) ||
      peer.peerId.toLowerCase().includes(query) ||
      peer.addresses.some((addr) => addr.toLowerCase().includes(query))
    );
  });

  // Handle adding a custom relay server
  const handleAddCustomRelay = async () => {
    if (!customRelayAddress.trim()) {
      toast.error('Please enter a relay address');
      return;
    }

    // Basic validation for relay multiaddress format
    if (!customRelayAddress.includes('/p2p/')) {
      toast.error('Relay address must include peer ID (/p2p/...)');
      return;
    }

    setIsAddingRelay(true);
    try {
      await addBootstrapNode(customRelayAddress.trim());
      toast.success('Custom relay added! Connecting...');
      setCustomRelayAddress('');
    } catch (err) {
      console.error('Failed to add relay:', err);
      toast.error(`Failed to add relay: ${err}`);
    } finally {
      setIsAddingRelay(false);
    }
  };

  // Handle adding a contact by peer ID
  const handleAddContactByPeerId = async () => {
    if (!manualPeerId.trim()) {
      toast.error('Please enter a Peer ID');
      return;
    }

    // Basic validation: libp2p peer IDs typically start with "12D3KooW" and are ~52 chars
    if (!manualPeerId.startsWith('12D3KooW') || manualPeerId.length < 50) {
      toast.error(
        "Invalid Peer ID format. It should start with '12D3KooW' and be about 52 characters.",
      );
      return;
    }

    setIsAddingContact(true);
    try {
      await contactsService.requestPeerIdentity(manualPeerId.trim());
      toast.success('Identity request sent! Contact will be added when they respond.');
      setShowAddContactModal(false);
      setManualPeerId('');
    } catch (err) {
      console.error('Failed to request identity:', err);
      toast.error(`Failed to add contact: ${err}`);
    } finally {
      setIsAddingContact(false);
    }
  };

  return (
    <div className="h-full flex flex-col" style={{ background: 'hsl(var(--harbor-bg-primary))' }}>
      {/* Header */}
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
              Manage your connections and contacts
            </p>
          </div>

          {/* Network toggle button */}
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

          {/* Network Status Card */}
          <div
            className="rounded-2xl p-6"
            style={{
              background: 'hsl(var(--harbor-bg-elevated))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
            }}
          >
            <div className="flex items-start gap-4">
              {/* Status indicator */}
              <div
                className="w-14 h-14 rounded-xl flex items-center justify-center flex-shrink-0"
                style={{
                  background:
                    status === 'connected'
                      ? 'hsl(var(--harbor-success) / 0.15)'
                      : status === 'connecting'
                        ? 'hsl(var(--harbor-warning) / 0.15)'
                        : 'hsl(var(--harbor-surface-2))',
                }}
              >
                <NetworkIcon
                  className="w-7 h-7"
                  style={{
                    color:
                      status === 'connected'
                        ? 'hsl(var(--harbor-success))'
                        : status === 'connecting'
                          ? 'hsl(var(--harbor-warning))'
                          : 'hsl(var(--harbor-text-tertiary))',
                  }}
                />
              </div>

              <div className="flex-1">
                <div className="flex items-center gap-2 mb-1">
                  <h3
                    className="text-lg font-semibold"
                    style={{ color: 'hsl(var(--harbor-text-primary))' }}
                  >
                    Peer-to-Peer Network
                  </h3>
                  <div
                    className="px-2 py-0.5 rounded-full text-xs font-medium"
                    style={{
                      background:
                        status === 'connected'
                          ? 'hsl(var(--harbor-success) / 0.15)'
                          : status === 'connecting'
                            ? 'hsl(var(--harbor-warning) / 0.15)'
                            : 'hsl(var(--harbor-surface-2))',
                      color:
                        status === 'connected'
                          ? 'hsl(var(--harbor-success))'
                          : status === 'connecting'
                            ? 'hsl(var(--harbor-warning))'
                            : 'hsl(var(--harbor-text-tertiary))',
                    }}
                  >
                    {status === 'connected'
                      ? 'Online'
                      : status === 'connecting'
                        ? 'Connecting...'
                        : 'Offline'}
                  </div>
                </div>
                <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                  {isRunning
                    ? `Connected to ${stats.connectedPeers} peer${stats.connectedPeers !== 1 ? 's' : ''} via libp2p`
                    : 'Start the network to discover and connect with peers'}
                </p>
              </div>
            </div>

            {/* Stats grid */}
            {isRunning && (
              <div className="grid grid-cols-2 md:grid-cols-5 gap-3 mt-6">
                <div
                  className="p-3 rounded-xl"
                  style={{ background: 'hsl(var(--harbor-surface-1))' }}
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
                  style={{ background: 'hsl(var(--harbor-surface-1))' }}
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
                  style={{ background: 'hsl(var(--harbor-surface-1))' }}
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
                  style={{ background: 'hsl(var(--harbor-surface-1))' }}
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
                {/* NAT Status indicator */}
                <div
                  className="p-3 rounded-xl"
                  style={{
                    background:
                      stats.natStatus === 'public'
                        ? 'hsl(var(--harbor-success) / 0.1)'
                        : stats.natStatus === 'private'
                          ? 'hsl(var(--harbor-warning) / 0.1)'
                          : 'hsl(var(--harbor-surface-1))',
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
                        ? 'Detecting...'
                        : stats.natStatus === 'public'
                          ? 'Public'
                          : stats.natStatus === 'private'
                            ? 'Relayed'
                            : 'Behind NAT'}
                    </p>
                  </div>
                </div>
              </div>
            )}
          </div>

          {/* Identity Card */}
          {identity && (
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
                Your Identity
              </h3>

              <div className="flex items-center gap-4">
                <div
                  className="w-16 h-16 rounded-full flex items-center justify-center text-xl font-semibold text-white flex-shrink-0"
                  style={{
                    background:
                      'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                  }}
                >
                  {identity.avatarHash ? (
                    <img
                      src={`/media/${identity.avatarHash}`}
                      alt=""
                      className="w-full h-full rounded-full object-cover"
                    />
                  ) : (
                    getInitials(identity.displayName)
                  )}
                </div>

                <div className="flex-1 min-w-0">
                  <p
                    className="text-lg font-semibold"
                    style={{ color: 'hsl(var(--harbor-text-primary))' }}
                  >
                    {identity.displayName}
                  </p>
                  {identity.bio && (
                    <p
                      className="text-sm mt-0.5"
                      style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                    >
                      {identity.bio}
                    </p>
                  )}
                  <div
                    className="mt-2 px-3 py-1.5 rounded-lg inline-block font-mono text-xs"
                    style={{
                      background: 'hsl(var(--harbor-surface-1))',
                      color: 'hsl(var(--harbor-text-tertiary))',
                    }}
                  >
                    {identity.peerId.slice(0, 16)}...{identity.peerId.slice(-12)}
                  </div>
                </div>

                <button
                  className="p-2 rounded-lg transition-colors duration-200"
                  style={{
                    color: 'hsl(var(--harbor-text-tertiary))',
                    background: 'hsl(var(--harbor-surface-1))',
                  }}
                  title="Copy Peer ID"
                  onClick={() => {
                    navigator.clipboard.writeText(identity.peerId);
                    toast.success('Peer ID copied to clipboard!');
                  }}
                >
                  <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={1.5}
                      d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
                    />
                  </svg>
                </button>
              </div>
            </div>
          )}

          {/* Connect to Remote Peers Card - Only show when network is running */}
          {isRunning && (
            <div
              className="rounded-2xl p-6"
              style={{
                background: 'hsl(var(--harbor-bg-elevated))',
                border: '1px solid hsl(var(--harbor-border-subtle))',
              }}
            >
              <div className="flex items-center justify-between mb-4">
                <h3
                  className="text-sm font-medium"
                  style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                >
                  Remote Connections
                </h3>
                <button
                  onClick={() => setShowConnectPeerModal(true)}
                  className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm font-medium transition-all"
                  style={{
                    background:
                      'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                    color: 'white',
                  }}
                >
                  <PlusIcon className="w-4 h-4" />
                  Connect to Peer
                </button>
              </div>

              {/* Your addresses for sharing */}
              {listeningAddresses.length > 0 && (
                <div>
                  {/* Show relay addresses prominently if available */}
                  {stats.relayAddresses && stats.relayAddresses.length > 0 && (
                    <div className="mb-4">
                      <label
                        className="text-xs font-medium block mb-2 flex items-center gap-2"
                        style={{ color: 'hsl(var(--harbor-success))' }}
                      >
                        <span className="w-2 h-2 rounded-full animate-pulse" style={{ background: 'hsl(var(--harbor-success))' }} />
                        Relay Address (works anywhere)
                      </label>
                      {stats.relayAddresses.map((addr, idx) => (
                        <div
                          key={`relay-${idx}`}
                          className="flex items-center gap-2 p-3 rounded-lg mb-2"
                          style={{
                            background: 'hsl(var(--harbor-success) / 0.1)',
                            border: '1px solid hsl(var(--harbor-success) / 0.2)',
                          }}
                        >
                          <code
                            className="text-xs flex-1 break-all font-mono"
                            style={{ color: 'hsl(var(--harbor-success))' }}
                          >
                            {addr}
                          </code>
                          <button
                            className="p-1.5 rounded hover:bg-white/10 flex-shrink-0"
                            onClick={() => {
                              navigator.clipboard.writeText(addr);
                              toast.success('Relay address copied! Share this with remote peers.');
                            }}
                            title="Copy relay address"
                          >
                            <svg
                              className="w-4 h-4"
                              fill="none"
                              stroke="currentColor"
                              viewBox="0 0 24 24"
                              style={{ color: 'hsl(var(--harbor-success))' }}
                            >
                              <path
                                strokeLinecap="round"
                                strokeLinejoin="round"
                                strokeWidth={2}
                                d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
                              />
                            </svg>
                          </button>
                        </div>
                      ))}
                      <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                        This address works for peers anywhere on the internet, not just your local network.
                      </p>
                    </div>
                  )}

                  {/* Show local addresses */}
                  <label
                    className="text-xs font-medium block mb-2"
                    style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                  >
                    Local network addresses
                  </label>
                  <div className="space-y-2">
                    {listeningAddresses
                      .filter((addr) => !addr.includes('127.0.0.1') && !addr.includes('::1') && !addr.includes('p2p-circuit'))
                      .slice(0, 3)
                      .map((addr, idx) => (
                        <div
                          key={idx}
                          className="flex items-center gap-2 p-2 rounded-lg"
                          style={{ background: 'hsl(var(--harbor-surface-1))' }}
                        >
                          <code
                            className="text-xs flex-1 break-all font-mono"
                            style={{ color: 'hsl(var(--harbor-primary))' }}
                          >
                            {addr}
                          </code>
                          <button
                            className="p-1.5 rounded hover:bg-white/10 flex-shrink-0"
                            onClick={() => {
                              navigator.clipboard.writeText(addr);
                              toast.success('Address copied!');
                            }}
                            title="Copy address"
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
                        </div>
                      ))}
                  </div>
                  <p className="text-xs mt-2" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                    These addresses only work for peers on your local network (same WiFi/LAN).
                  </p>
                </div>
              )}

              {listeningAddresses.length === 0 && (
                <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                  Addresses will appear here once the network is ready.
                </p>
              )}
            </div>
          )}

          {/* Peers/Contacts Section */}
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
                  onClick={() => setActiveTab('peers')}
                  className="px-4 py-1.5 rounded-md text-sm font-medium transition-all duration-200"
                  style={{
                    background:
                      activeTab === 'peers' ? 'hsl(var(--harbor-bg-elevated))' : 'transparent',
                    color:
                      activeTab === 'peers'
                        ? 'hsl(var(--harbor-text-primary))'
                        : 'hsl(var(--harbor-text-tertiary))',
                    boxShadow: activeTab === 'peers' ? 'var(--shadow-sm)' : 'none',
                  }}
                >
                  <UsersIcon className="w-4 h-4 inline mr-1.5" />
                  Peers
                </button>
                <button
                  onClick={() => setActiveTab('contacts')}
                  className="px-4 py-1.5 rounded-md text-sm font-medium transition-all duration-200"
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
                  <UserIcon className="w-4 h-4 inline mr-1.5" />
                  Contacts
                </button>
              </div>

              <div className="flex-1 relative">
                <SearchIcon
                  className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4"
                  style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                />
                <input
                  type="text"
                  placeholder="Search peers..."
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="w-full pl-10 pr-4 py-2 rounded-lg text-sm"
                  style={{
                    background: 'hsl(var(--harbor-surface-1))',
                    border: '1px solid hsl(var(--harbor-border-subtle))',
                    color: 'hsl(var(--harbor-text-primary))',
                  }}
                />
              </div>

              <button
                className="p-2 rounded-lg transition-colors duration-200"
                style={{
                  background:
                    'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                  color: 'white',
                }}
                title="Add contact by Peer ID"
                onClick={() => setShowAddContactModal(true)}
              >
                <PlusIcon className="w-5 h-5" />
              </button>
            </div>

            {/* Peer list */}
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
                    Start the network to discover peers on your local network
                  </p>
                </div>
              ) : filteredPeers.length === 0 ? (
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
                    {searchQuery ? 'No peers found' : 'No peers connected'}
                  </p>
                  <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                    {searchQuery
                      ? 'Try a different search term'
                      : 'Peers running Harbor on your network will appear here'}
                  </p>
                </div>
              ) : (
                <div className="space-y-2">
                  {filteredPeers.map((peer) => {
                    const friendlyName = getPeerFriendlyName(peer.peerId);
                    const avatarColor = getPeerColor(peer.peerId);
                    const initials = friendlyName
                      .split(' ')
                      .map((w) => w[0])
                      .join('');

                    return (
                      <div
                        key={peer.peerId}
                        className="flex items-center gap-4 p-3 rounded-xl transition-all duration-200"
                        style={{
                          background: 'hsl(var(--harbor-surface-1))',
                        }}
                      >
                        {/* Avatar with color gradient */}
                        <div
                          className="w-10 h-10 rounded-full flex items-center justify-center text-sm font-semibold text-white flex-shrink-0"
                          style={{
                            background: avatarColor,
                          }}
                        >
                          {initials}
                        </div>

                        {/* Info - friendly name prominently, peer ID small */}
                        <div className="flex-1 min-w-0">
                          <p
                            className="font-medium text-sm"
                            style={{ color: 'hsl(var(--harbor-text-primary))' }}
                          >
                            {friendlyName}
                          </p>
                          <p
                            className="text-xs font-mono truncate"
                            style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                            title={peer.peerId}
                          >
                            {peer.peerId.slice(0, 12)}...{peer.peerId.slice(-6)}
                          </p>
                        </div>

                        {/* Status */}
                        <div className="flex items-center gap-2">
                          <div
                            className={`w-2 h-2 rounded-full ${
                              peer.isConnected ? 'animate-pulse' : ''
                            }`}
                            style={{
                              background: peer.isConnected
                                ? 'hsl(var(--harbor-success))'
                                : 'hsl(var(--harbor-text-tertiary))',
                            }}
                          />
                          <span
                            className="text-xs"
                            style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                          >
                            {peer.isConnected ? 'Connected' : 'Disconnected'}
                          </span>
                        </div>

                        {/* Actions */}
                        <button
                          className="p-2 rounded-lg transition-colors duration-200"
                          style={{
                            background: 'hsl(var(--harbor-success) / 0.15)',
                            color: 'hsl(var(--harbor-success))',
                          }}
                          title={`Add ${friendlyName} to contacts`}
                          onClick={async () => {
                            try {
                              await contactsService.requestPeerIdentity(peer.peerId);
                              toast.success(`Requesting identity from ${friendlyName}...`);
                            } catch (e) {
                              toast.error(`Failed to add contact: ${e}`);
                            }
                          }}
                        >
                          <CheckIcon className="w-4 h-4" />
                        </button>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          </div>

          {/* Advanced Section - Collapsible */}
          <div
            className="rounded-2xl p-6"
            style={{
              background: 'hsl(var(--harbor-bg-elevated))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
            }}
          >
            <button
              onClick={() => setShowAdvanced(!showAdvanced)}
              className="flex items-center gap-2 text-sm font-medium transition-colors"
              style={{ color: 'hsl(var(--harbor-text-secondary))' }}
            >
              <svg
                className={`w-4 h-4 transition-transform ${showAdvanced ? 'rotate-90' : ''}`}
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
              </svg>
              Advanced: Deploy Your Own Relay Server
            </button>

            {showAdvanced && (
              <div className="mt-4">
            <div className="flex items-start gap-4 mb-6">
              <div
                className="w-12 h-12 rounded-xl flex items-center justify-center flex-shrink-0"
                style={{ background: 'hsl(var(--harbor-primary) / 0.15)' }}
              >
                <svg
                  className="w-6 h-6"
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
                <h3
                  className="text-lg font-semibold mb-1"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  Deploy Your Own Relay Server
                </h3>
                <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                  Run your own relay server on AWS for better connectivity. Free tier eligible!
                </p>
              </div>
            </div>

            {/* Warning about existing deployments */}
            <div
              className="mb-6 p-3 rounded-lg flex items-start gap-3"
              style={{ background: 'hsl(var(--harbor-warning) / 0.1)' }}
            >
              <svg
                className="w-5 h-5 flex-shrink-0 mt-0.5"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
                style={{ color: 'hsl(var(--harbor-warning))' }}
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
                />
              </svg>
              <div className="text-xs" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                <strong style={{ color: 'hsl(var(--harbor-warning))' }}>Before deploying:</strong>{' '}
                Check if you already have a relay running to avoid duplicate charges.{' '}
                <a
                  href="https://console.aws.amazon.com/cloudformation/home#/stacks?filteringStatus=active&filteringText=harbor-relay"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="underline font-medium"
                  style={{ color: 'hsl(var(--harbor-primary))' }}
                >
                  Check existing stacks →
                </a>
              </div>
            </div>

            {/* Deploy to AWS - Step by Step */}
            <div className="mb-6">
              <label
                className="text-xs font-medium block mb-3"
                style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
              >
                Deploy to AWS (Free for 12 months)
              </label>

              {/* Step 1: Download */}
              <div className="mb-4 p-4 rounded-xl" style={{ background: 'hsl(var(--harbor-surface-1))' }}>
                <div className="flex items-center gap-3 mb-3">
                  <span
                    className="w-6 h-6 rounded-full flex items-center justify-center text-xs font-bold"
                    style={{
                      background: 'linear-gradient(135deg, #FF9900, #FF6600)',
                      color: 'white',
                    }}
                  >
                    1
                  </span>
                  <span className="text-sm font-medium" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
                    Download the template file
                  </span>
                </div>
                <button
                  onClick={() => {
                    const blob = new Blob([RELAY_CLOUDFORMATION_TEMPLATE], { type: 'application/x-yaml' });
                    const url = URL.createObjectURL(blob);
                    const a = document.createElement('a');
                    a.href = url;
                    a.download = 'harbor-relay-cloudformation.yaml';
                    document.body.appendChild(a);
                    a.click();
                    document.body.removeChild(a);
                    URL.revokeObjectURL(url);
                    toast.success('Template downloaded! Continue to Step 2.');
                  }}
                  className="flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-all hover:scale-105"
                  style={{
                    background: 'linear-gradient(135deg, #FF9900, #FF6600)',
                    color: 'white',
                  }}
                >
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
                  </svg>
                  Download Template File
                </button>
              </div>

              {/* Step 2: Open AWS */}
              <div className="mb-4 p-4 rounded-xl" style={{ background: 'hsl(var(--harbor-surface-1))' }}>
                <div className="flex items-center gap-3 mb-3">
                  <span
                    className="w-6 h-6 rounded-full flex items-center justify-center text-xs font-bold"
                    style={{
                      background: 'linear-gradient(135deg, #FF9900, #FF6600)',
                      color: 'white',
                    }}
                  >
                    2
                  </span>
                  <span className="text-sm font-medium" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
                    Open AWS and upload the template
                  </span>
                </div>
                <a
                  href="https://console.aws.amazon.com/cloudformation/home#/stacks/create/template"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-colors mb-3"
                  style={{
                    background: 'hsl(var(--harbor-primary))',
                    color: 'white',
                  }}
                >
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
                  </svg>
                  Open AWS CloudFormation
                </a>
                <div className="text-xs space-y-1" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                  <p>• Select <strong>"Upload a template file"</strong></p>
                  <p>• Click <strong>"Choose file"</strong> and select the downloaded file</p>
                  <p>• Click <strong>"Next"</strong></p>
                </div>
              </div>

              {/* Step 3: Configure Stack */}
              <div className="mb-4 p-4 rounded-xl" style={{ background: 'hsl(var(--harbor-surface-1))' }}>
                <div className="flex items-center gap-3 mb-3">
                  <span
                    className="w-6 h-6 rounded-full flex items-center justify-center text-xs font-bold"
                    style={{
                      background: 'linear-gradient(135deg, #FF9900, #FF6600)',
                      color: 'white',
                    }}
                  >
                    3
                  </span>
                  <span className="text-sm font-medium" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
                    Fill in the settings (easy!)
                  </span>
                </div>
                <div className="text-xs space-y-2" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                  <div className="p-2 rounded" style={{ background: 'hsl(var(--harbor-bg-primary))' }}>
                    <p><strong>Stack name:</strong> <code className="font-mono px-1 rounded" style={{ background: 'hsl(var(--harbor-surface-2))' }}>harbor-relay</code></p>
                    <p className="text-xs opacity-75 mt-1">This is just a label to identify your server in AWS</p>
                  </div>
                  <div className="p-2 rounded" style={{ background: 'hsl(var(--harbor-bg-primary))' }}>
                    <p><strong>InstanceType:</strong> Leave as <code className="font-mono px-1 rounded" style={{ background: 'hsl(var(--harbor-surface-2))' }}>t2.micro</code> (free!)</p>
                  </div>
                  <div className="p-2 rounded" style={{ background: 'hsl(var(--harbor-bg-primary))' }}>
                    <p><strong>KeyPairName:</strong> <span className="font-medium" style={{ color: 'hsl(var(--harbor-success))' }}>Leave empty</span></p>
                    <p className="text-xs opacity-75 mt-1">You don't need SSH access - we'll use AWS's built-in browser terminal</p>
                  </div>
                  <div className="p-2 rounded" style={{ background: 'hsl(var(--harbor-bg-primary))' }}>
                    <p><strong>All other settings:</strong> Leave as default</p>
                  </div>
                  <p className="pt-2">Click <strong>"Next"</strong> to continue</p>
                </div>
              </div>

              {/* Step 4: Review and Create */}
              <div className="mb-4 p-4 rounded-xl" style={{ background: 'hsl(var(--harbor-surface-1))' }}>
                <div className="flex items-center gap-3 mb-3">
                  <span
                    className="w-6 h-6 rounded-full flex items-center justify-center text-xs font-bold"
                    style={{
                      background: 'linear-gradient(135deg, #FF9900, #FF6600)',
                      color: 'white',
                    }}
                  >
                    4
                  </span>
                  <span className="text-sm font-medium" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
                    Review and create
                  </span>
                </div>
                <div className="text-xs space-y-2" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                  <p>• On the "Configure stack options" page: just click <strong>"Next"</strong></p>
                  <p>• On the "Review" page: scroll to the bottom</p>
                  <div className="p-2 rounded flex items-start gap-2" style={{ background: 'hsl(var(--harbor-warning) / 0.1)' }}>
                    <input type="checkbox" disabled className="mt-0.5" />
                    <p><strong>Check the box</strong> that says "I acknowledge that AWS CloudFormation might create IAM resources with custom names"</p>
                  </div>
                  <p>• Click <strong>"Submit"</strong> or <strong>"Create stack"</strong></p>
                  <p className="pt-2" style={{ color: 'hsl(var(--harbor-success))' }}>
                    <strong>Wait 3-5 minutes</strong> for the server to start up. You'll see "CREATE_COMPLETE" when ready!
                  </p>
                </div>
              </div>
            </div>

            {/* Step 5: Get Your Relay Address (after deployment) */}
            <div className="mb-4 p-4 rounded-xl" style={{ background: 'hsl(var(--harbor-success) / 0.1)', border: '1px solid hsl(var(--harbor-success) / 0.2)' }}>
              <div className="flex items-center gap-3 mb-3">
                <span
                  className="w-6 h-6 rounded-full flex items-center justify-center text-xs font-bold"
                  style={{
                    background: 'hsl(var(--harbor-success))',
                    color: 'white',
                  }}
                >
                  5
                </span>
                <span className="text-sm font-medium" style={{ color: 'hsl(var(--harbor-success))' }}>
                  Get your relay address (after 5 minutes)
                </span>
              </div>
              <div className="text-xs space-y-3" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                <p>After deployment completes, your relay address is automatically saved to AWS Parameter Store.</p>

                <div className="p-3 rounded-lg" style={{ background: 'hsl(var(--harbor-bg-primary))' }}>
                  <p className="font-medium mb-2" style={{ color: 'hsl(var(--harbor-text-primary))' }}>To find your relay address:</p>
                  <ol className="space-y-1 list-decimal list-inside">
                    <li>Go to your CloudFormation stack's <strong>"Outputs"</strong> tab</li>
                    <li>Click the link next to <strong>"Step2GetYourRelayAddress"</strong></li>
                    <li>Copy the <strong>"Value"</strong> field (starts with <code className="font-mono">/ip4/...</code>)</li>
                  </ol>
                </div>

                <a
                  href="https://console.aws.amazon.com/cloudformation/home#/stacks?filteringStatus=active&filteringText=harbor-relay"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-colors"
                  style={{
                    background: 'hsl(var(--harbor-success))',
                    color: 'white',
                  }}
                >
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />
                  </svg>
                  Open Your CloudFormation Stack
                </a>

                <p className="pt-1">Once you have the address, paste it below and click <strong>"Add Relay"</strong></p>
              </div>
            </div>

            {/* Add Custom Relay */}
            <div>
              <label
                className="text-xs font-medium block mb-2"
                style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
              >
                Add Your Custom Relay
              </label>
              <div className="flex gap-2">
                <input
                  type="text"
                  value={customRelayAddress}
                  onChange={(e) => setCustomRelayAddress(e.target.value)}
                  placeholder="/ip4/YOUR_IP/tcp/4001/p2p/YOUR_PEER_ID"
                  className="flex-1 px-3 py-2 rounded-lg text-sm font-mono"
                  style={{
                    background: 'hsl(var(--harbor-surface-1))',
                    border: '1px solid hsl(var(--harbor-border-subtle))',
                    color: 'hsl(var(--harbor-text-primary))',
                  }}
                  disabled={isAddingRelay || !isRunning}
                />
                <button
                  onClick={handleAddCustomRelay}
                  disabled={isAddingRelay || !customRelayAddress.trim() || !isRunning}
                  className="px-4 py-2 rounded-lg font-medium text-sm transition-colors disabled:opacity-50"
                  style={{
                    background: 'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                    color: 'white',
                  }}
                >
                  {isAddingRelay ? 'Adding...' : 'Add Relay'}
                </button>
              </div>
              {!isRunning && (
                <p className="text-xs mt-2" style={{ color: 'hsl(var(--harbor-warning))' }}>
                  Start the network first to add a custom relay.
                </p>
              )}
              <p className="text-xs mt-2" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                Format: <code className="font-mono">/ip4/PUBLIC_IP/tcp/4001/p2p/PEER_ID</code>
              </p>
            </div>

            {/* Cost Info */}
            <div
              className="mt-6 p-3 rounded-lg flex items-start gap-3"
              style={{ background: 'hsl(var(--harbor-success) / 0.1)' }}
            >
              <svg
                className="w-5 h-5 flex-shrink-0 mt-0.5"
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
              <div className="text-xs" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                <strong style={{ color: 'hsl(var(--harbor-success))' }}>Free for 12 months!</strong>{' '}
                AWS Free Tier includes 750 hours/month of t2.micro - enough to run one relay 24/7.
                After the free tier: ~$9-12/month.
              </div>
            </div>

            {/* Cleanup Section */}
            <div
              className="mt-4 pt-4 border-t"
              style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
            >
              <div className="flex items-center justify-between">
                <div className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                  <strong>Need to delete your relay?</strong> Remove all resources to stop charges.
                </div>
                <a
                  href="https://console.aws.amazon.com/cloudformation/home#/stacks?filteringStatus=active&filteringText=harbor-relay"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors"
                  style={{
                    background: 'hsl(var(--harbor-error) / 0.1)',
                    color: 'hsl(var(--harbor-error))',
                  }}
                >
                  <svg
                    className="w-3.5 h-3.5"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
                    />
                  </svg>
                  Manage Stacks
                </a>
              </div>
              <p
                className="text-xs mt-2"
                style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
              >
                Select your stack and click "Delete" to remove all resources.
              </p>
            </div>
          </div>
        )}
          </div>
        </div>
      </div>

      {/* Add Contact by Peer ID Modal */}
      {showAddContactModal && (
        <div
          className="fixed inset-0 flex items-center justify-center z-50"
          style={{ background: 'rgba(0, 0, 0, 0.6)' }}
          onClick={() => setShowAddContactModal(false)}
        >
          <div
            className="rounded-xl p-6 max-w-md w-full mx-4"
            style={{ background: 'hsl(var(--harbor-bg-elevated))' }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center justify-between mb-4">
              <h2
                className="text-lg font-semibold"
                style={{ color: 'hsl(var(--harbor-text-primary))' }}
              >
                Add Contact by Peer ID
              </h2>
              <button
                onClick={() => setShowAddContactModal(false)}
                className="p-1 rounded-lg hover:bg-white/10"
              >
                <XIcon className="w-5 h-5" style={{ color: 'hsl(var(--harbor-text-secondary))' }} />
              </button>
            </div>

            <p className="text-sm mb-4" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
              Enter the Peer ID of the person you want to add. You can find your own Peer ID in
              Settings.
            </p>

            {/* Show own Peer ID for easy sharing */}
            {identity && (
              <div
                className="mb-4 p-3 rounded-lg"
                style={{ background: 'hsl(var(--harbor-surface-1))' }}
              >
                <label
                  className="text-xs font-medium block mb-1"
                  style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                >
                  Your Peer ID (share this with others)
                </label>
                <div className="flex items-center gap-2">
                  <code
                    className="text-xs flex-1 break-all"
                    style={{ color: 'hsl(var(--harbor-primary))' }}
                  >
                    {identity.peerId}
                  </code>
                  <button
                    className="p-1 rounded hover:bg-white/10"
                    onClick={() => {
                      navigator.clipboard.writeText(identity.peerId);
                      toast.success('Peer ID copied!');
                    }}
                    title="Copy Peer ID"
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
                </div>
              </div>
            )}

            <input
              type="text"
              value={manualPeerId}
              onChange={(e) => setManualPeerId(e.target.value)}
              placeholder="Enter Peer ID (starts with 12D3KooW...)"
              className="w-full p-3 rounded-lg text-sm mb-4"
              style={{
                background: 'hsl(var(--harbor-surface-1))',
                border: '1px solid hsl(var(--harbor-border-subtle))',
                color: 'hsl(var(--harbor-text-primary))',
              }}
              disabled={isAddingContact}
            />

            <div className="flex gap-3">
              <button
                onClick={() => setShowAddContactModal(false)}
                className="flex-1 px-4 py-2 rounded-lg font-medium transition-colors"
                style={{
                  background: 'hsl(var(--harbor-surface-1))',
                  color: 'hsl(var(--harbor-text-primary))',
                }}
                disabled={isAddingContact}
              >
                Cancel
              </button>
              <button
                onClick={handleAddContactByPeerId}
                disabled={isAddingContact || !manualPeerId.trim()}
                className="flex-1 px-4 py-2 rounded-lg font-medium transition-colors disabled:opacity-50"
                style={{
                  background:
                    'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                  color: 'white',
                }}
              >
                {isAddingContact ? 'Adding...' : 'Add Contact'}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Connect to Peer Modal */}
      {showConnectPeerModal && (
        <div
          className="fixed inset-0 flex items-center justify-center z-50"
          style={{ background: 'rgba(0, 0, 0, 0.6)' }}
          onClick={() => setShowConnectPeerModal(false)}
        >
          <div
            className="rounded-xl p-6 max-w-lg w-full mx-4"
            style={{ background: 'hsl(var(--harbor-bg-elevated))' }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center justify-between mb-4">
              <h2
                className="text-lg font-semibold"
                style={{ color: 'hsl(var(--harbor-text-primary))' }}
              >
                Connect to Remote Peer
              </h2>
              <button
                onClick={() => setShowConnectPeerModal(false)}
                className="p-1 rounded-lg hover:bg-white/10"
              >
                <XIcon className="w-5 h-5" style={{ color: 'hsl(var(--harbor-text-secondary))' }} />
              </button>
            </div>

            <p className="text-sm mb-4" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
              Enter the multiaddress of the peer you want to connect to. They can find their address
              in the Network page under "Remote Connections".
            </p>

            <div
              className="mb-4 p-3 rounded-lg"
              style={{ background: 'hsl(var(--harbor-surface-1))' }}
            >
              <label
                className="text-xs font-medium block mb-1"
                style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
              >
                Example format
              </label>
              <code
                className="text-xs break-all"
                style={{ color: 'hsl(var(--harbor-text-secondary))' }}
              >
                /ip4/1.2.3.4/tcp/9000/p2p/12D3KooW...
              </code>
            </div>

            <input
              type="text"
              value={peerMultiaddr}
              onChange={(e) => setPeerMultiaddr(e.target.value)}
              placeholder="Enter multiaddress..."
              className="w-full p-3 rounded-lg text-sm mb-4 font-mono"
              style={{
                background: 'hsl(var(--harbor-surface-1))',
                border: '1px solid hsl(var(--harbor-border-subtle))',
                color: 'hsl(var(--harbor-text-primary))',
              }}
              disabled={isConnecting}
            />

            <div className="flex gap-3">
              <button
                onClick={() => setShowConnectPeerModal(false)}
                className="flex-1 px-4 py-2 rounded-lg font-medium transition-colors"
                style={{
                  background: 'hsl(var(--harbor-surface-1))',
                  color: 'hsl(var(--harbor-text-primary))',
                }}
                disabled={isConnecting}
              >
                Cancel
              </button>
              <button
                onClick={handleConnectToPeer}
                disabled={isConnecting || !peerMultiaddr.trim()}
                className="flex-1 px-4 py-2 rounded-lg font-medium transition-colors disabled:opacity-50"
                style={{
                  background:
                    'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                  color: 'white',
                }}
              >
                {isConnecting ? 'Connecting...' : 'Connect'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
