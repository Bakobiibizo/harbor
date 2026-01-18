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
  } = useNetworkStore();

  const [searchQuery, setSearchQuery] = useState('');
  const [activeTab, setActiveTab] = useState<'peers' | 'contacts'>('peers');
  const [showAddContactModal, setShowAddContactModal] = useState(false);
  const [showConnectPeerModal, setShowConnectPeerModal] = useState(false);
  const [manualPeerId, setManualPeerId] = useState('');
  const [peerMultiaddr, setPeerMultiaddr] = useState('');
  const [isAddingContact, setIsAddingContact] = useState(false);
  const [isConnecting, setIsConnecting] = useState(false);

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
              <div className="grid grid-cols-2 md:grid-cols-4 gap-3 mt-6">
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
                  <label
                    className="text-xs font-medium block mb-2"
                    style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                  >
                    Your shareable addresses (for remote peers)
                  </label>
                  <div className="space-y-2">
                    {listeningAddresses
                      .filter((addr) => !addr.includes('127.0.0.1') && !addr.includes('::1'))
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
                    Share one of these addresses with someone to let them connect to you directly.
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
