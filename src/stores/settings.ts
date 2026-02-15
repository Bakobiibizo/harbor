import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export type ThemeMode = 'system' | 'light' | 'dark';
export type FontSize = 'small' | 'medium' | 'large';
export type AccentColor =
  | 'blue'
  | 'purple'
  | 'green'
  | 'orange'
  | 'pink'
  | 'red'
  | 'teal'
  | 'amber';

// Accent color definitions using HSL values
export const ACCENT_COLORS: Record<
  AccentColor,
  { primary: string; accent: string; label: string; swatch: string }
> = {
  blue: {
    primary: '220 91% 54%',
    accent: '262 83% 58%',
    label: 'Blue',
    swatch: '#3b82f6',
  },
  purple: {
    primary: '262 83% 58%',
    accent: '280 73% 53%',
    label: 'Purple',
    swatch: '#8b5cf6',
  },
  green: {
    primary: '152 69% 40%',
    accent: '170 60% 45%',
    label: 'Green',
    swatch: '#22c55e',
  },
  orange: {
    primary: '25 95% 53%',
    accent: '38 92% 50%',
    label: 'Orange',
    swatch: '#f97316',
  },
  pink: {
    primary: '330 81% 60%',
    accent: '350 80% 55%',
    label: 'Pink',
    swatch: '#ec4899',
  },
  red: {
    primary: '0 84% 60%',
    accent: '15 80% 55%',
    label: 'Red',
    swatch: '#ef4444',
  },
  teal: {
    primary: '175 70% 41%',
    accent: '190 65% 50%',
    label: 'Teal',
    swatch: '#14b8a6',
  },
  amber: {
    primary: '38 92% 50%',
    accent: '45 93% 47%',
    label: 'Amber',
    swatch: '#f59e0b',
  },
};

// Font size scale multipliers
const FONT_SIZE_SCALES: Record<FontSize, number> = {
  small: 0.875,
  medium: 1,
  large: 1.125,
};

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
  accentColor: AccentColor;
  fontSize: FontSize;

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
  setAccentColor: (value: AccentColor) => void;
  setFontSize: (value: FontSize) => void;
}

function applyTheme(theme: ThemeMode) {
  const root = document.documentElement;
  if (theme === 'system') {
    root.removeAttribute('data-theme');
  } else {
    root.setAttribute('data-theme', theme);
  }
}

function applyAccentColor(color: AccentColor) {
  const root = document.documentElement;
  const colorDef = ACCENT_COLORS[color];
  root.style.setProperty('--harbor-primary', colorDef.primary);
  root.style.setProperty('--harbor-accent', colorDef.accent);
  // Also update the lighter and darker variants based on primary
  const hslParts = colorDef.primary.split(' ');
  if (hslParts.length === 3) {
    const h = hslParts[0];
    const s = hslParts[1];
    const lStr = hslParts[2].replace('%', '');
    const l = parseFloat(lStr);
    root.style.setProperty('--harbor-primary-light', `${h} ${s} ${Math.min(l + 10, 100)}%`);
    root.style.setProperty('--harbor-primary-dark', `${h} ${s} ${Math.max(l - 10, 0)}%`);
  }
}

function applyFontSize(size: FontSize) {
  const root = document.documentElement;
  const scale = FONT_SIZE_SCALES[size];
  root.style.setProperty('--harbor-font-scale', String(scale));
  // Apply base font size to html element
  root.style.fontSize = `${scale * 100}%`;
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
      accentColor: 'blue',
      fontSize: 'medium',

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
      setAccentColor: (value) => {
        applyAccentColor(value);
        set({ accentColor: value });
      },
      setFontSize: (value) => {
        applyFontSize(value);
        set({ fontSize: value });
      },
    }),
    {
      name: 'harbor-settings',
      onRehydrateStorage: () => {
        return (state: SettingsState | undefined) => {
          if (state?.theme) {
            applyTheme(state.theme);
          }
          if (state?.accentColor) {
            applyAccentColor(state.accentColor);
          }
          if (state?.fontSize) {
            applyFontSize(state.fontSize);
          }
        };
      },
    },
  ),
);
