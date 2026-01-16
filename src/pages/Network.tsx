import { useEffect } from "react";
import { useIdentityStore, useNetworkStore } from "../stores";

export function NetworkPage() {
  const { state } = useIdentityStore();
  const {
    isRunning,
    status,
    connectedPeers,
    stats,
    error,
    isLoading,
    startNetwork,
    stopNetwork,
    refreshPeers,
    refreshStats,
    checkStatus,
  } = useNetworkStore();

  const identity = state.status === "unlocked" ? state.identity : null;

  // Check network status on mount and set up refresh interval
  useEffect(() => {
    checkStatus();

    // Refresh peers and stats every 5 seconds when running
    const interval = setInterval(() => {
      if (isRunning) {
        refreshPeers();
        refreshStats();
      }
    }, 5000);

    return () => clearInterval(interval);
  }, [isRunning, checkStatus, refreshPeers, refreshStats]);

  const formatBytes = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  const formatUptime = (seconds: number) => {
    const hours = Math.floor(seconds / 3600);
    const mins = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;
    if (hours > 0) return `${hours}h ${mins}m`;
    if (mins > 0) return `${mins}m ${secs}s`;
    return `${secs}s`;
  };

  return (
    <div className="h-full p-6 overflow-y-auto">
      <div className="max-w-2xl mx-auto">
        <h2 className="text-2xl font-bold text-gray-900 dark:text-white mb-6">
          Network
        </h2>

        {/* Network Status Card */}
        <div className="bg-white dark:bg-gray-800 rounded-xl shadow p-6 mb-6">
          <div className="flex items-center justify-between">
            <div>
              <h3 className="text-lg font-semibold text-gray-900 dark:text-white">
                P2P Network
              </h3>
              <p className="text-sm mt-1">
                {status === "connected" && (
                  <span className="text-green-600 dark:text-green-400">Connected</span>
                )}
                {status === "connecting" && (
                  <span className="text-yellow-600 dark:text-yellow-400">Connecting...</span>
                )}
                {status === "disconnected" && (
                  <span className="text-gray-500 dark:text-gray-400">Disconnected</span>
                )}
              </p>
            </div>
            <button
              onClick={isRunning ? stopNetwork : startNetwork}
              disabled={isLoading || state.status !== "unlocked"}
              className={`px-4 py-2 rounded-lg font-medium transition-colors ${
                isRunning
                  ? "bg-red-600 hover:bg-red-700 text-white"
                  : "bg-blue-600 hover:bg-blue-700 text-white"
              } ${isLoading || state.status !== "unlocked" ? "opacity-50 cursor-not-allowed" : ""}`}
            >
              {isLoading
                ? "..."
                : isRunning
                  ? "Stop Network"
                  : "Start Network"}
            </button>
          </div>

          {state.status !== "unlocked" && !isRunning && (
            <p className="text-sm text-gray-500 dark:text-gray-400 mt-2">
              Unlock your identity to start the network
            </p>
          )}
        </div>

        {error && (
          <div className="bg-red-100 dark:bg-red-900/50 border border-red-300 dark:border-red-500 rounded-lg p-4 mb-6">
            <p className="text-red-700 dark:text-red-400">{error}</p>
          </div>
        )}

        {/* Stats */}
        {isRunning && (
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-6">
            <div className="bg-white dark:bg-gray-800 rounded-xl shadow p-4">
              <p className="text-sm text-gray-500 dark:text-gray-400">Connected Peers</p>
              <p className="text-2xl font-bold text-gray-900 dark:text-white">{stats.connectedPeers}</p>
            </div>
            <div className="bg-white dark:bg-gray-800 rounded-xl shadow p-4">
              <p className="text-sm text-gray-500 dark:text-gray-400">Uptime</p>
              <p className="text-2xl font-bold text-gray-900 dark:text-white">{formatUptime(stats.uptimeSeconds)}</p>
            </div>
            <div className="bg-white dark:bg-gray-800 rounded-xl shadow p-4">
              <p className="text-sm text-gray-500 dark:text-gray-400">Data In</p>
              <p className="text-2xl font-bold text-gray-900 dark:text-white">{formatBytes(stats.totalBytesIn)}</p>
            </div>
            <div className="bg-white dark:bg-gray-800 rounded-xl shadow p-4">
              <p className="text-sm text-gray-500 dark:text-gray-400">Data Out</p>
              <p className="text-2xl font-bold text-gray-900 dark:text-white">{formatBytes(stats.totalBytesOut)}</p>
            </div>
          </div>
        )}

        {/* Identity card */}
        {identity && (
          <div className="bg-white dark:bg-gray-800 rounded-xl shadow p-6 mb-6">
            <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
              Your Identity
            </h3>

            <div className="space-y-3">
              <div>
                <label className="text-sm text-gray-500 dark:text-gray-400">
                  Display Name
                </label>
                <p className="text-gray-900 dark:text-white font-medium">
                  {identity.displayName}
                </p>
              </div>

              {identity.bio && (
                <div>
                  <label className="text-sm text-gray-500 dark:text-gray-400">
                    Bio
                  </label>
                  <p className="text-gray-900 dark:text-white">
                    {identity.bio}
                  </p>
                </div>
              )}

              <div>
                <label className="text-sm text-gray-500 dark:text-gray-400">
                  Peer ID
                </label>
                <p className="text-xs font-mono text-gray-700 dark:text-gray-300 break-all bg-gray-100 dark:bg-gray-700 p-2 rounded">
                  {identity.peerId}
                </p>
              </div>
            </div>
          </div>
        )}

        {/* Connected Peers */}
        <div className="bg-white dark:bg-gray-800 rounded-xl shadow p-6">
          <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
            {isRunning ? "Connected Peers" : "Contacts"}
          </h3>

          {!isRunning ? (
            <div className="text-center py-8">
              <div className="text-4xl mb-2">🌐</div>
              <p className="text-gray-600 dark:text-gray-400">
                Start the network to discover and connect to peers
              </p>
            </div>
          ) : connectedPeers.length === 0 ? (
            <div className="text-center py-8">
              <div className="text-4xl mb-2">📡</div>
              <p className="text-gray-600 dark:text-gray-400">
                No peers connected yet. Other devices on your network running this app will appear here.
              </p>
            </div>
          ) : (
            <div className="space-y-2">
              {connectedPeers.map((peer) => (
                <div
                  key={peer.peerId}
                  className="bg-gray-50 dark:bg-gray-700 rounded-lg p-3 flex items-center justify-between"
                >
                  <div className="overflow-hidden">
                    <p className="font-mono text-sm truncate text-gray-900 dark:text-white">
                      {peer.peerId}
                    </p>
                    <p className="text-xs text-gray-500 dark:text-gray-400 truncate">
                      {peer.addresses.length > 0
                        ? peer.addresses[0]
                        : "No address"}
                    </p>
                  </div>
                  <div
                    className={`w-2 h-2 rounded-full flex-shrink-0 ml-2 ${
                      peer.isConnected ? "bg-green-500" : "bg-gray-400"
                    }`}
                  />
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
