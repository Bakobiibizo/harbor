import { describe, it, expect, beforeEach } from 'vitest';
import { useSettingsStore } from './settings';

describe('useSettingsStore', () => {
  beforeEach(() => {
    localStorage.clear();
    // Reset to defaults
    useSettingsStore.setState({
      soundEnabled: true,
      autoStartNetwork: true,
      localDiscovery: true,
      bootstrapNodes: [],
      iceServers: [],
      showReadReceipts: true,
      showOnlineStatus: true,
      defaultVisibility: 'contacts',
      avatarUrl: null,
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

      const stored = JSON.parse(localStorage.getItem('harbor-settings') ?? '{}');

      expect(stored.state.iceServers[0]).not.toHaveProperty('credential');
      expect(stored.state.iceServers[1].credential).toBe('device-secret');
      expect(useSettingsStore.getState().iceServers[0].credential).toBe('session-secret');
    });
  });

  describe('privacy settings', () => {
    it('should toggle showReadReceipts', () => {
      useSettingsStore.getState().setShowReadReceipts(false);
      expect(useSettingsStore.getState().showReadReceipts).toBe(false);
    });

    it('should toggle showOnlineStatus', () => {
      useSettingsStore.getState().setShowOnlineStatus(false);
      expect(useSettingsStore.getState().showOnlineStatus).toBe(false);
    });

    it('should set defaultVisibility', () => {
      useSettingsStore.getState().setDefaultVisibility('public');
      expect(useSettingsStore.getState().defaultVisibility).toBe('public');

      useSettingsStore.getState().setDefaultVisibility('contacts');
      expect(useSettingsStore.getState().defaultVisibility).toBe('contacts');
    });
  });

  describe('profile settings', () => {
    it('should set avatar URL', () => {
      useSettingsStore.getState().setAvatarUrl('blob:http://localhost/abc123');
      expect(useSettingsStore.getState().avatarUrl).toBe('blob:http://localhost/abc123');
    });

    it('should clear avatar URL', () => {
      useSettingsStore.getState().setAvatarUrl('blob:http://localhost/abc123');
      useSettingsStore.getState().setAvatarUrl(null);
      expect(useSettingsStore.getState().avatarUrl).toBeNull();
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
        '50 100% 47%',
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
      expect(state.showReadReceipts).toBe(true);
      expect(state.showOnlineStatus).toBe(true);
      expect(state.defaultVisibility).toBe('contacts');
      expect(state.avatarUrl).toBeNull();
      expect(state.theme).toBe('system');
      expect(state.accentColor).toBe('harbor');
      expect(state.fontSize).toBe('medium');
    });
  });
});
