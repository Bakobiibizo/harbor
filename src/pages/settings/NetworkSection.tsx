import { useState } from 'react';
import toast from 'react-hot-toast';
import { useSettingsStore } from '../../stores';
import { Toggle } from './shared';

export function NetworkSection() {
  const {
    autoStartNetwork,
    localDiscovery,
    bootstrapNodes,
    setAutoStartNetwork,
    setLocalDiscovery,
    addBootstrapNode,
    removeBootstrapNode,
  } = useSettingsStore();

  const [newNodeAddress, setNewNodeAddress] = useState('');

  const handleAddNode = () => {
    const address = newNodeAddress.trim();
    if (!address) {
      toast.error('Please enter a relay address');
      return;
    }
    if (bootstrapNodes.includes(address)) {
      toast.error('This address is already added');
      return;
    }
    addBootstrapNode(address);
    setNewNodeAddress('');
    toast.success('Relay node added');
  };

  const handleRemoveNode = (address: string) => {
    removeBootstrapNode(address);
    toast.success('Relay node removed');
  };

  return (
    <div className="space-y-6">
      <div>
        <h3
          className="text-xl font-semibold mb-1"
          style={{ color: 'hsl(var(--harbor-text-primary))' }}
        >
          Network
        </h3>
        <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
          Configure peer-to-peer networking and discovery
        </p>
      </div>

      {/* Auto-start network */}
      <div
        className="rounded-lg p-6"
        style={{
          background: 'hsl(var(--harbor-bg-elevated))',
          border: '1px solid hsl(var(--harbor-border-subtle))',
        }}
      >
        <div className="flex items-center justify-between">
          <div>
            <h4 className="font-medium" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
              Auto-start network
            </h4>
            <p className="text-sm mt-0.5" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
              Automatically connect to the peer-to-peer network on launch
            </p>
          </div>
          <Toggle
            enabled={autoStartNetwork}
            onChange={(value) => {
              setAutoStartNetwork(value);
              toast.success(
                value ? 'Network will auto-start on launch' : 'Network will not auto-start',
              );
            }}
          />
        </div>
      </div>

      {/* Local discovery */}
      <div
        className="rounded-lg p-6"
        style={{
          background: 'hsl(var(--harbor-bg-elevated))',
          border: '1px solid hsl(var(--harbor-border-subtle))',
        }}
      >
        <div className="flex items-center justify-between">
          <div>
            <h4 className="font-medium" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
              Local discovery (mDNS)
            </h4>
            <p className="text-sm mt-0.5" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
              Discover peers on your local network automatically
            </p>
          </div>
          <Toggle
            enabled={localDiscovery}
            onChange={(value) => {
              setLocalDiscovery(value);
              toast.success(value ? 'Local discovery enabled' : 'Local discovery disabled');
            }}
          />
        </div>
      </div>

      {/* Bootstrap / Relay nodes */}
      <div
        className="rounded-lg p-6"
        style={{
          background: 'hsl(var(--harbor-bg-elevated))',
          border: '1px solid hsl(var(--harbor-border-subtle))',
        }}
      >
        <h4 className="font-medium mb-2" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
          Relay Nodes
        </h4>
        <p className="text-sm mb-4" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
          Add custom relay addresses to help connect with peers behind NAT
        </p>

        {/* Add new node */}
        <div className="flex gap-2 mb-4">
          <input
            type="text"
            value={newNodeAddress}
            onChange={(e) => setNewNodeAddress(e.target.value)}
            placeholder="/ip4/1.2.3.4/tcp/4001/p2p/12D3..."
            onKeyDown={(e) => {
              if (e.key === 'Enter') handleAddNode();
            }}
            className="flex-1 px-4 py-3 rounded-lg text-sm font-mono"
            style={{
              background: 'hsl(var(--harbor-surface-1))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
              color: 'hsl(var(--harbor-text-primary))',
            }}
          />
          <button
            onClick={handleAddNode}
            className="px-4 py-3 rounded-lg text-sm font-medium"
            style={{
              background:
                'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
              color: 'white',
            }}
          >
            Add
          </button>
        </div>

        {/* Node list */}
        {bootstrapNodes.length > 0 ? (
          <div className="space-y-2">
            {bootstrapNodes.map((address) => (
              <div
                key={address}
                className="flex items-center gap-2 p-3 rounded-lg"
                style={{
                  background: 'hsl(var(--harbor-surface-1))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                }}
              >
                <span
                  className="flex-1 text-sm font-mono truncate"
                  style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                >
                  {address}
                </span>
                <button
                  onClick={() => handleRemoveNode(address)}
                  className="p-1 rounded"
                  style={{ color: 'hsl(var(--harbor-error))' }}
                >
                  <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M6 18L18 6M6 6l12 12"
                    />
                  </svg>
                </button>
              </div>
            ))}
          </div>
        ) : (
          <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
            No custom relay nodes configured. The default community relay will be used.
          </p>
        )}
      </div>
    </div>
  );
}
