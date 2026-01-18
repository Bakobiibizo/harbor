import { create } from 'zustand';
import { persist } from 'zustand/middleware';

interface SettingsState {
  // Network settings
  autoStartNetwork: boolean;
  localDiscovery: boolean;
  bootstrapNodes: string[];

  // Privacy settings
  showReadReceipts: boolean;
  showOnlineStatus: boolean;
  defaultVisibility: 'contacts' | 'public';

  // Profile settings
  avatarUrl: string | null;

  // Actions
  setAutoStartNetwork: (value: boolean) => void;
  setLocalDiscovery: (value: boolean) => void;
  addBootstrapNode: (address: string) => void;
  removeBootstrapNode: (address: string) => void;
  setShowReadReceipts: (value: boolean) => void;
  setShowOnlineStatus: (value: boolean) => void;
  setDefaultVisibility: (value: 'contacts' | 'public') => void;
  setAvatarUrl: (url: string | null) => void;
}

export const useSettingsStore = create<SettingsState>()(
  persist(
    (set) => ({
      // Initial values
      autoStartNetwork: true,
      localDiscovery: true,
      bootstrapNodes: [],
      showReadReceipts: true,
      showOnlineStatus: true,
      defaultVisibility: 'contacts',
      avatarUrl: null,

      // Actions
      setAutoStartNetwork: (value) => set({ autoStartNetwork: value }),
      setLocalDiscovery: (value) => set({ localDiscovery: value }),
      addBootstrapNode: (address) =>
        set((state) => ({
          bootstrapNodes: state.bootstrapNodes.includes(address)
            ? state.bootstrapNodes
            : [...state.bootstrapNodes, address],
        })),
      removeBootstrapNode: (address) =>
        set((state) => ({
          bootstrapNodes: state.bootstrapNodes.filter((a) => a !== address),
        })),
      setShowReadReceipts: (value) => set({ showReadReceipts: value }),
      setShowOnlineStatus: (value) => set({ showOnlineStatus: value }),
      setDefaultVisibility: (value) => set({ defaultVisibility: value }),
      setAvatarUrl: (url) => set({ avatarUrl: url }),
    }),
    {
      name: 'harbor-settings',
    },
  ),
);
