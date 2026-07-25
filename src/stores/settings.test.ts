import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useSettingsStore } from './settings';
import { grantProviderSessionConsent, hasProviderSessionConsent } from '../utils/providerEmbeds';
import { activateProfile, suspendProfile } from '../services/profileSession';
import { getMessagingPrivacyPolicy, setReadReceiptsEnabled } from '../services/messagingPrivacy';

vi.mock('../services/messagingPrivacy', () => ({
  getMessagingPrivacyPolicy: vi.fn(),
  setReadReceiptsEnabled: vi.fn(),
}));

const PROFILE_SETTINGS_KEY = 'harbor:profile:test-profile:settings:v3';

describe('useSettingsStore', () => {
  beforeEach(() => {
    vi.mocked(getMessagingPrivacyPolicy).mockResolvedValue({ readReceiptsEnabled: false });
    vi.mocked(setReadReceiptsEnabled).mockImplementation(async (enabled) => ({
      readReceiptsEnabled: enabled,
    }));
    suspendProfile();
    activateProfile('test-profile');
    localStorage.clear();
    // Reset to defaults
    useSettingsStore.setState({
      soundEnabled: true,
      autoStartNetwork: true,
      localDiscovery: true,
      bootstrapNodes: [],
      iceServers: [],
      showReadReceipts: false,
      readReceiptsStatus: 'ready',
      readReceiptsError: null,
      defaultVisibility: 'contacts',
      socialView: 'all',
      communityView: 'all',
      providerEmbedConsent: 'per-use',
      theme: 'system',
      accentColor: 'harbor',
      fontSize: 'medium',
    });
  });

  describe('network settings', () => {
    it('should toggle autoStartNetwork', () => {
      useSettingsStore.getState().setAutoStartNetwork(false);
      expect(useSettingsStore.getState().autoStartNetwork).toBe(false);

      useSettingsStore.getState().setAutoStartNetwork(true);
      expect(useSettingsStore.getState().autoStartNetwork).toBe(true);
    });

    it('should toggle localDiscovery', () => {
      useSettingsStore.getState().setLocalDiscovery(false);
      expect(useSettingsStore.getState().localDiscovery).toBe(false);
    });

    it('should add bootstrap nodes without duplicates', () => {
      const { addBootstrapNode } = useSettingsStore.getState();
      addBootstrapNode('/ip4/1.2.3.4/tcp/9000');
      addBootstrapNode('/ip4/5.6.7.8/tcp/9000');
      addBootstrapNode('/ip4/1.2.3.4/tcp/9000'); // duplicate

      expect(useSettingsStore.getState().bootstrapNodes).toHaveLength(2);
      expect(useSettingsStore.getState().bootstrapNodes).toContain('/ip4/1.2.3.4/tcp/9000');
      expect(useSettingsStore.getState().bootstrapNodes).toContain('/ip4/5.6.7.8/tcp/9000');
    });

    it('should remove bootstrap nodes', () => {
      useSettingsStore.getState().addBootstrapNode('/ip4/1.2.3.4/tcp/9000');
      useSettingsStore.getState().addBootstrapNode('/ip4/5.6.7.8/tcp/9000');

      useSettingsStore.getState().removeBootstrapNode('/ip4/1.2.3.4/tcp/9000');

      expect(useSettingsStore.getState().bootstrapNodes).toEqual(['/ip4/5.6.7.8/tcp/9000']);
    });

    it('should handle removing a node that does not exist', () => {
      useSettingsStore.getState().addBootstrapNode('/ip4/1.2.3.4/tcp/9000');
      useSettingsStore.getState().removeBootstrapNode('/ip4/nonexistent/tcp/9000');

      expect(useSettingsStore.getState().bootstrapNodes).toEqual(['/ip4/1.2.3.4/tcp/9000']);
    });

    it('should add validated ICE servers and reject invalid entries', () => {
      const { addIceServer } = useSettingsStore.getState();

      const stun = addIceServer({ urls: 'stun:stun.example.test:3478' });
      const turn = addIceServer({
        urls: 'turn:turn.example.test:3478?transport=udp',
        username: 'operator',
        credential: 'secret',
      });

      expect(useSettingsStore.getState().iceServers).toEqual([stun, turn]);
      expect(turn.credentialPersistence).toBe('session');
      expect(() => addIceServer({ urls: 'https://invalid.example.test' })).toThrow(/stun:|turn:/);
    });

    it('should redact ICE server credentials for display', () => {
      const { addIceServer, getRedactedIceServers } = useSettingsStore.getState();

      addIceServer({
        urls: 'turn:turn.example.test:3478',
        username: 'operator',
        credential: 'secret',
      });

      expect(getRedactedIceServers()).toEqual([
        expect.objectContaining({
          username: 'operator',
          hasCredential: true,
          redactedCredential: '••••••••',
        }),
      ]);
    });

    it('should remove ICE servers', () => {
      const server = useSettingsStore
        .getState()
        .addIceServer({ urls: 'stun:stun.example.test:3478' });

      useSettingsStore.getState().removeIceServer(server.id);

      expect(useSettingsStore.getState().iceServers).toEqual([]);
    });

    it('should persist device TURN credentials but not session-only TURN credentials', () => {
      useSettingsStore.getState().addIceServer({
        urls: 'turn:session.example.test:3478',
        username: 'session-user',
        credential: 'session-secret',
        credentialPersistence: 'session',
      });
      useSettingsStore.getState().addIceServer({
        urls: 'turn:device.example.test:3478',
        username: 'device-user',
        credential: 'device-secret',
        credentialPersistence: 'device',
      });

      const stored = JSON.parse(localStorage.getItem(PROFILE_SETTINGS_KEY) ?? '{}');

      expect(stored.state.iceServers[0]).not.toHaveProperty('credential');
      expect(stored.state.iceServers[1].credential).toBe('device-secret');
      expect(useSettingsStore.getState().iceServers[0].credential).toBe('session-secret');
    });
  });

  describe('privacy settings', () => {
    it('links the feed and personal wall filter while keeping communities independent', () => {
      useSettingsStore.getState().setSocialView('videos');
      expect(useSettingsStore.getState().socialView).toBe('videos');
      expect(useSettingsStore.getState().communityView).toBe('all');

      useSettingsStore.getState().setCommunityView('audio');
      expect(useSettingsStore.getState().socialView).toBe('videos');
      expect(useSettingsStore.getState().communityView).toBe('audio');
    });

    it('reflects the authoritative read receipt policy only after persistence succeeds', async () => {
      let release!: (value: { readReceiptsEnabled: boolean }) => void;
      vi.mocked(setReadReceiptsEnabled).mockReturnValueOnce(
        new Promise((resolve) => {
          release = resolve;
        }),
      );

      const update = useSettingsStore.getState().setShowReadReceipts(true);
      expect(useSettingsStore.getState()).toMatchObject({
        showReadReceipts: false,
        readReceiptsStatus: 'loading',
      });

      release({ readReceiptsEnabled: true });
      await update;
      expect(useSettingsStore.getState()).toMatchObject({
        showReadReceipts: true,
        readReceiptsStatus: 'ready',
      });
    });

    it('keeps the previous policy and rejects when backend persistence fails', async () => {
      vi.mocked(setReadReceiptsEnabled).mockRejectedValueOnce({
        code: 'DATABASE_ERROR',
        message: 'Policy storage failed',
      });

      await expect(useSettingsStore.getState().setShowReadReceipts(true)).rejects.toMatchObject({
        code: 'DATABASE_ERROR',
      });
      expect(useSettingsStore.getState()).toMatchObject({
        showReadReceipts: false,
        readReceiptsStatus: 'error',
        readReceiptsError: 'Policy storage failed',
      });
    });

    it('never treats browser storage as the read-receipt authority', async () => {
      await useSettingsStore.getState().setShowReadReceipts(true);
      useSettingsStore.getState().setDefaultVisibility('public');

      const stored = JSON.parse(localStorage.getItem(PROFILE_SETTINGS_KEY) ?? '{}');
      expect(stored.state).not.toHaveProperty('showReadReceipts');
      expect(stored.state).not.toHaveProperty('showOnlineStatus');
      expect(setReadReceiptsEnabled).toHaveBeenCalledWith(true);
    });

    it('should set defaultVisibility', () => {
      useSettingsStore.getState().setDefaultVisibility('public');
      expect(useSettingsStore.getState().defaultVisibility).toBe('public');

      useSettingsStore.getState().setDefaultVisibility('contacts');
      expect(useSettingsStore.getState().defaultVisibility).toBe('contacts');
    });

    it('requires per-use provider consent by default and accepts an explicit session setting', () => {
      expect(useSettingsStore.getState().providerEmbedConsent).toBe('per-use');
      useSettingsStore.getState().setProviderEmbedConsent('session');
      grantProviderSessionConsent('youtube');
      expect(hasProviderSessionConsent('youtube')).toBe(true);
      expect(useSettingsStore.getState().providerEmbedConsent).toBe('session');
      useSettingsStore.getState().setProviderEmbedConsent('per-use');
      expect(useSettingsStore.getState().providerEmbedConsent).toBe('per-use');
      expect(hasProviderSessionConsent('youtube')).toBe(false);
    });

    it('persists the consent policy but never persists session provider grants', () => {
      useSettingsStore.getState().setProviderEmbedConsent('session');
      grantProviderSessionConsent('spotify');

      const stored = JSON.parse(localStorage.getItem(PROFILE_SETTINGS_KEY) ?? '{}');
      expect(stored.state.providerEmbedConsent).toBe('session');
      expect(JSON.stringify(stored)).not.toContain('spotify');
    });
  });

  describe('appearance settings', () => {
    it('should set theme', () => {
      useSettingsStore.getState().setTheme('dark');
      expect(useSettingsStore.getState().theme).toBe('dark');

      useSettingsStore.getState().setTheme('light');
      expect(useSettingsStore.getState().theme).toBe('light');

      useSettingsStore.getState().setTheme('system');
      expect(useSettingsStore.getState().theme).toBe('system');
    });

    it('should set accent color', () => {
      useSettingsStore.getState().setAccentColor('harbor');
      expect(useSettingsStore.getState().accentColor).toBe('harbor');
      expect(document.documentElement.style.getPropertyValue('--harbor-primary')).toBe(
        '214 81% 47%',
      );
      expect(document.documentElement.style.getPropertyValue('--harbor-accent')).toBe(
        '214 81% 47%',
      );

      useSettingsStore.getState().setAccentColor('purple');
      expect(useSettingsStore.getState().accentColor).toBe('purple');

      useSettingsStore.getState().setAccentColor('green');
      expect(useSettingsStore.getState().accentColor).toBe('green');
    });

    it('should set font size', () => {
      useSettingsStore.getState().setFontSize('small');
      expect(useSettingsStore.getState().fontSize).toBe('small');

      useSettingsStore.getState().setFontSize('large');
      expect(useSettingsStore.getState().fontSize).toBe('large');
    });
  });

  describe('default values', () => {
    it('should have correct default state', () => {
      const state = useSettingsStore.getState();
      expect(state.autoStartNetwork).toBe(true);
      expect(state.localDiscovery).toBe(true);
      expect(state.bootstrapNodes).toEqual([]);
      expect(state.iceServers).toEqual([]);
      expect(state.showReadReceipts).toBe(false);
      expect(state.readReceiptsStatus).toBe('ready');
      expect(state.defaultVisibility).toBe('contacts');
      expect(state.socialView).toBe('all');
      expect(state.communityView).toBe('all');
      expect(state.providerEmbedConsent).toBe('per-use');
      expect(state.theme).toBe('system');
      expect(state.accentColor).toBe('harbor');
      expect(state.fontSize).toBe('medium');
    });
  });
});
