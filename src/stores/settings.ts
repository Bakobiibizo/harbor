import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export type ThemeMode = 'system' | 'light' | 'dark';

interface SettingsState {
  // Network settings
  autoStartNetwork: boolean;
  localDiscovery: boolean;
  bootstrapNodes: string[];

  // Notification settings
  soundEnabled: boolean;

  // Privacy settings
  showReadReceipts: boolean;
  showOnlineStatus: boolean;
  defaultVisibility: 'contacts' | 'public';

  // Profile settings
  avatarUrl: string | null;

  // Appearance settings
  theme: ThemeMode;

  // Actions
  setSoundEnabled: (value: boolean) => void;
  setAutoStartNetwork: (value: boolean) => void;
  setLocalDiscovery: (value: boolean) => void;
  addBootstrapNode: (address: string) => void;
  removeBootstrapNode: (address: string) => void;
  setShowReadReceipts: (value: boolean) => void;
  setShowOnlineStatus: (value: boolean) => void;
  setDefaultVisibility: (value: 'contacts' | 'public') => void;
  setAvatarUrl: (url: string | null) => void;
  setTheme: (value: ThemeMode) => void;
}

function applyTheme(theme: ThemeMode) {
  const root = document.documentElement;
  if (theme === 'system') {
    root.removeAttribute('data-theme');
  } else {
    root.setAttribute('data-theme', theme);
  }
}

export const useSettingsStore = create<SettingsState>()(
  persist(
    (set) => ({
      // Initial values
      soundEnabled: true,
      autoStartNetwork: true,
      localDiscovery: true,
      bootstrapNodes: [],
      showReadReceipts: true,
      showOnlineStatus: true,
      defaultVisibility: 'contacts',
      avatarUrl: null,
      theme: 'system',

      // Actions
      setSoundEnabled: (value) => set({ soundEnabled: value }),
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
      setTheme: (value) => {
        applyTheme(value);
        set({ theme: value });
      },
    }),
    {
      name: 'harbor-settings',
      onRehydrateStorage: () => {
        return (state: SettingsState | undefined) => {
          if (state?.theme) {
            applyTheme(state.theme);
          }
        };
      },
    },
  ),
);
