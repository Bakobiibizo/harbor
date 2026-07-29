import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import {
  stripSessionCredentialsForPersistence,
  validateIceServerInput,
} from '../services/callingIce';
import type { IceServerConfig, IceServerInput, RedactedIceServerConfig } from '../types';
import { clearProviderSessionConsent, type ProviderEmbedConsent } from '../utils/providerEmbeds';
import { requireProfileId } from '../services/profileSession';
import { migrateLegacyProfileValue, profileStorageKey } from '../services/profileStorage';
import { getMessagingPrivacyPolicy, setReadReceiptsEnabled } from '../services/messagingPrivacy';
import { getErrorMessage } from '../utils/errors';

export type ThemeMode = 'system' | 'light' | 'dark';
export type FontSize = 'small' | 'medium' | 'large';
export type SocialView = 'all' | 'images' | 'videos' | 'audio';
export type AccentColor =
  'harbor' | 'blue' | 'purple' | 'green' | 'orange' | 'pink' | 'red' | 'teal' | 'amber';

// Accent color definitions using HSL values
export const ACCENT_COLORS: Record<
  AccentColor,
  { primary: string; accent: string; label: string; swatch: string }
> = {
  harbor: {
    primary: '214 81% 47%',
    accent: '214 81% 47%',
    label: 'Harbor',
    swatch: '#1769D7',
  },
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

const SETTINGS_LEGACY_KEY = 'harbor-settings';
const DEVICE_APPEARANCE_KEY = 'harbor-device-appearance-v1';
const SETTINGS_PROFILE_NAMESPACE = 'settings';
const SETTINGS_PROFILE_VERSION = 3;

type PostVisibility = 'contacts' | 'public';

interface ProfileSettings {
  soundEnabled: boolean;
  autoStartNetwork: boolean;
  localDiscovery: boolean;
  bootstrapNodes: string[];
  contactFeedPollingEnabled: boolean;
  contactFeedPollIntervalMinutes: number;
  iceServers: IceServerConfig[];
  defaultVisibility: PostVisibility;
  socialView: SocialView;
  communityView: SocialView;
  providerEmbedConsent: ProviderEmbedConsent;
}

interface DeviceAppearance {
  theme: ThemeMode;
  accentColor: AccentColor;
  fontSize: FontSize;
}

const profileSettingsDefaults: ProfileSettings = {
  soundEnabled: true,
  autoStartNetwork: true,
  localDiscovery: true,
  bootstrapNodes: [] as string[],
  contactFeedPollingEnabled: true,
  contactFeedPollIntervalMinutes: 5,
  iceServers: [] as IceServerConfig[],
  defaultVisibility: 'contacts',
  socialView: 'all',
  communityView: 'all',
  providerEmbedConsent: 'per-use',
};

const deviceAppearanceDefaults: DeviceAppearance = {
  theme: 'system',
  accentColor: 'harbor',
  fontSize: 'medium',
};

function profileSettingsSnapshot(state: SettingsState): ProfileSettings {
  return {
    soundEnabled: state.soundEnabled,
    autoStartNetwork: state.autoStartNetwork,
    localDiscovery: state.localDiscovery,
    bootstrapNodes: state.bootstrapNodes,
    contactFeedPollingEnabled: state.contactFeedPollingEnabled,
    contactFeedPollIntervalMinutes: state.contactFeedPollIntervalMinutes,
    iceServers: state.iceServers.map(stripSessionCredentialsForPersistence),
    defaultVisibility: state.defaultVisibility,
    socialView: state.socialView,
    communityView: state.communityView,
    providerEmbedConsent: state.providerEmbedConsent,
  };
}

function writeProfileSettings(state: SettingsState): void {
  localStorage.setItem(
    profileStorageKey(SETTINGS_PROFILE_NAMESPACE, SETTINGS_PROFILE_VERSION),
    JSON.stringify({ state: profileSettingsSnapshot(state), version: SETTINGS_PROFILE_VERSION }),
  );
}

function persistedState(raw: string | null): Record<string, unknown> {
  if (!raw) return {};
  try {
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    return parsed.state && typeof parsed.state === 'object'
      ? (parsed.state as Record<string, unknown>)
      : parsed;
  } catch {
    return {};
  }
}

function normalizeView(value: unknown): SocialView {
  return value === 'images' || value === 'videos' || value === 'audio' ? value : 'all';
}

function readLegacyDeviceAppearance(): DeviceAppearance | null {
  if (localStorage.getItem(DEVICE_APPEARANCE_KEY)) return null;
  const legacyRaw = localStorage.getItem(SETTINGS_LEGACY_KEY);
  if (!legacyRaw) return null;

  const state = persistedState(legacyRaw);
  const appearance: DeviceAppearance = {
    theme:
      state.theme === 'light' || state.theme === 'dark' || state.theme === 'system'
        ? state.theme
        : deviceAppearanceDefaults.theme,
    accentColor:
      typeof state.accentColor === 'string' && state.accentColor in ACCENT_COLORS
        ? (state.accentColor as AccentColor)
        : deviceAppearanceDefaults.accentColor,
    fontSize:
      state.fontSize === 'small' || state.fontSize === 'large' || state.fontSize === 'medium'
        ? state.fontSize
        : deviceAppearanceDefaults.fontSize,
  };
  localStorage.setItem(DEVICE_APPEARANCE_KEY, JSON.stringify({ state: appearance, version: 1 }));
  return appearance;
}

function readProfileSettings(): ProfileSettings {
  const currentKey = profileStorageKey(SETTINGS_PROFILE_NAMESPACE, SETTINGS_PROFILE_VERSION);
  if (!localStorage.getItem(currentKey)) {
    const previousKey = profileStorageKey(SETTINGS_PROFILE_NAMESPACE, 2);
    const previous = localStorage.getItem(previousKey);
    if (previous) {
      const previousState = persistedState(previous);
      delete previousState.avatarUrl;
      localStorage.setItem(
        currentKey,
        JSON.stringify({ state: previousState, version: SETTINGS_PROFILE_VERSION }),
      );
      localStorage.removeItem(previousKey);
    }
  }
  const state = persistedState(
    migrateLegacyProfileValue(
      SETTINGS_LEGACY_KEY,
      SETTINGS_PROFILE_NAMESPACE,
      SETTINGS_PROFILE_VERSION,
    ),
  );
  return {
    ...profileSettingsDefaults,
    soundEnabled: state.soundEnabled !== false,
    autoStartNetwork: state.autoStartNetwork !== false,
    localDiscovery: state.localDiscovery !== false,
    bootstrapNodes: Array.isArray(state.bootstrapNodes)
      ? state.bootstrapNodes.filter((value): value is string => typeof value === 'string')
      : [],
    contactFeedPollingEnabled: state.contactFeedPollingEnabled !== false,
    contactFeedPollIntervalMinutes:
      typeof state.contactFeedPollIntervalMinutes === 'number'
        ? Math.min(30, Math.max(1, Math.round(state.contactFeedPollIntervalMinutes)))
        : 5,
    iceServers: Array.isArray(state.iceServers) ? (state.iceServers as IceServerConfig[]) : [],
    defaultVisibility: state.defaultVisibility === 'public' ? 'public' : 'contacts',
    socialView: normalizeView(state.socialView),
    communityView: normalizeView(state.communityView),
    providerEmbedConsent: state.providerEmbedConsent === 'session' ? 'session' : 'per-use',
  };
}

interface SettingsState {
  // Network settings
  autoStartNetwork: boolean;
  localDiscovery: boolean;
  bootstrapNodes: string[];
  contactFeedPollingEnabled: boolean;
  contactFeedPollIntervalMinutes: number;

  // Calling / WebRTC NAT traversal settings
  iceServers: IceServerConfig[];

  // Notification settings
  soundEnabled: boolean;

  // Privacy settings
  showReadReceipts: boolean;
  readReceiptsStatus: 'idle' | 'loading' | 'ready' | 'error';
  readReceiptsError: string | null;
  defaultVisibility: PostVisibility;
  socialView: SocialView;
  communityView: SocialView;
  providerEmbedConsent: ProviderEmbedConsent;

  // Appearance settings
  theme: ThemeMode;
  accentColor: AccentColor;
  fontSize: FontSize;

  // Actions
  setSoundEnabled: (value: boolean) => void;
  setAutoStartNetwork: (value: boolean) => void;
  setLocalDiscovery: (value: boolean) => void;
  setContactFeedPollingEnabled: (value: boolean) => void;
  setContactFeedPollIntervalMinutes: (value: number) => void;
  addBootstrapNode: (address: string) => void;
  removeBootstrapNode: (address: string) => void;
  addIceServer: (input: IceServerInput) => IceServerConfig;
  removeIceServer: (id: string) => void;
  getRedactedIceServers: () => RedactedIceServerConfig[];
  setShowReadReceipts: (value: boolean) => Promise<void>;
  setDefaultVisibility: (value: PostVisibility) => void;
  setSocialView: (value: SocialView) => void;
  setCommunityView: (value: SocialView) => void;
  setProviderEmbedConsent: (value: ProviderEmbedConsent) => void;
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
    (set, get) => {
      const commitProfile = (
        update: Partial<ProfileSettings> | ((state: SettingsState) => Partial<ProfileSettings>),
      ) => {
        requireProfileId();
        set(update);
        writeProfileSettings(get());
      };

      return {
        // Initial values
        ...profileSettingsDefaults,
        ...deviceAppearanceDefaults,
        showReadReceipts: false,
        readReceiptsStatus: 'idle',
        readReceiptsError: null,

        // Actions
        setSoundEnabled: (value) => commitProfile({ soundEnabled: value }),
        setAutoStartNetwork: (value) => commitProfile({ autoStartNetwork: value }),
        setLocalDiscovery: (value) => commitProfile({ localDiscovery: value }),
        setContactFeedPollingEnabled: (value) =>
          commitProfile({ contactFeedPollingEnabled: value }),
        setContactFeedPollIntervalMinutes: (value) =>
          commitProfile({
            contactFeedPollIntervalMinutes: Math.min(30, Math.max(1, Math.round(value))),
          }),
        addBootstrapNode: (address) =>
          commitProfile((state) => ({
            bootstrapNodes: state.bootstrapNodes.includes(address)
              ? state.bootstrapNodes
              : [...state.bootstrapNodes, address],
          })),
        removeBootstrapNode: (address) =>
          commitProfile((state) => ({
            bootstrapNodes: state.bootstrapNodes.filter((a) => a !== address),
          })),
        addIceServer: (input) => {
          const result = validateIceServerInput(input, get().iceServers);
          if (!result.ok) {
            throw new Error(result.error);
          }
          commitProfile((state) => ({ iceServers: [...state.iceServers, result.server] }));
          return result.server;
        },
        removeIceServer: (id) =>
          commitProfile((state) => ({
            iceServers: state.iceServers.filter((server) => server.id !== id),
          })),
        getRedactedIceServers: () =>
          get().iceServers.map((server) => ({
            id: server.id,
            urls: [...server.urls],
            username: server.username,
            credentialPersistence: server.credentialPersistence,
            hasCredential: Boolean(server.credential),
            redactedCredential: server.credential ? '••••••••' : undefined,
          })),
        setShowReadReceipts: async (value) => {
          set({ readReceiptsStatus: 'loading', readReceiptsError: null });
          try {
            const policy = await setReadReceiptsEnabled(value);
            set({
              showReadReceipts: policy.readReceiptsEnabled,
              readReceiptsStatus: 'ready',
            });
          } catch (error) {
            set({ readReceiptsStatus: 'error', readReceiptsError: getErrorMessage(error) });
            throw error;
          }
        },
        setDefaultVisibility: (value) => commitProfile({ defaultVisibility: value }),
        setSocialView: (value) => commitProfile({ socialView: value }),
        setCommunityView: (value) => commitProfile({ communityView: value }),
        setProviderEmbedConsent: (value) => {
          if (value === 'per-use') clearProviderSessionConsent();
          commitProfile({ providerEmbedConsent: value });
        },
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
      };
    },
    {
      name: DEVICE_APPEARANCE_KEY,
      version: 1,
      partialize: (state) => ({
        theme: state.theme,
        accentColor: state.accentColor,
        fontSize: state.fontSize,
      }),
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

export async function hydrateSettingsProfile(): Promise<void> {
  clearProviderSessionConsent();
  const migratedAppearance = readLegacyDeviceAppearance();
  useSettingsStore.setState({
    ...readProfileSettings(),
    ...(migratedAppearance ?? {}),
    showReadReceipts: false,
    readReceiptsStatus: 'loading',
    readReceiptsError: null,
  });
  if (migratedAppearance) {
    applyTheme(migratedAppearance.theme);
    applyAccentColor(migratedAppearance.accentColor);
    applyFontSize(migratedAppearance.fontSize);
  }
  try {
    const policy = await getMessagingPrivacyPolicy();
    useSettingsStore.setState({
      showReadReceipts: policy.readReceiptsEnabled,
      readReceiptsStatus: 'ready',
      readReceiptsError: null,
    });
  } catch (error) {
    useSettingsStore.setState({
      showReadReceipts: false,
      readReceiptsStatus: 'error',
      readReceiptsError: getErrorMessage(error),
    });
    throw error;
  }
}

export function resetSettingsProfileMemory(): void {
  clearProviderSessionConsent();
  const persistedAppearance =
    typeof localStorage === 'undefined' ? null : localStorage.getItem(DEVICE_APPEARANCE_KEY);
  useSettingsStore.setState({
    ...profileSettingsDefaults,
    showReadReceipts: false,
    readReceiptsStatus: 'idle',
    readReceiptsError: null,
  });
  if (typeof localStorage !== 'undefined') {
    if (persistedAppearance === null) {
      localStorage.removeItem(DEVICE_APPEARANCE_KEY);
    } else {
      localStorage.setItem(DEVICE_APPEARANCE_KEY, persistedAppearance);
    }
  }
}
