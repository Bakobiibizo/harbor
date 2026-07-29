import { beforeEach, describe, expect, it, vi } from 'vitest';
import { activateProfile, suspendProfile } from './profileSession';
import { hydrateProfilePersistence, resetProfilePersistenceMemory } from './profilePersistence';
import { useSettingsStore } from '../stores/settings';
import { useMessagingStore } from '../stores/messaging';
import { useNotificationsStore } from '../stores/notifications';
import { useFeedStore } from '../stores/feed';
import { getMessagingPrivacyPolicy } from './messagingPrivacy';

vi.mock('./messagingPrivacy', () => ({
  getMessagingPrivacyPolicy: vi.fn(),
  setReadReceiptsEnabled: vi.fn(),
}));

async function switchProfile(profileId: string): Promise<void> {
  suspendProfile();
  resetProfilePersistenceMemory();
  activateProfile(profileId);
  await hydrateProfilePersistence();
}

describe('profile persistence', () => {
  beforeEach(() => {
    vi.mocked(getMessagingPrivacyPolicy).mockResolvedValue({ readReceiptsEnabled: false });
    suspendProfile();
    resetProfilePersistenceMemory();
    useSettingsStore.setState({ theme: 'system', accentColor: 'harbor', fontSize: 'medium' });
    localStorage.clear();
    vi.stubGlobal('crypto', { randomUUID: vi.fn(() => 'notification-id') });
  });

  it('fails profile-owned writes before authoritative activation', () => {
    expect(() => useSettingsStore.getState().addBootstrapNode('/dns4/a.example/tcp/443')).toThrow(
      'No Harbor profile is active',
    );
    expect(() => useMessagingStore.getState().archiveConversation('peer-a')).toThrow(
      'No Harbor profile is active',
    );
    expect(() => useNotificationsStore.getState().clear()).toThrow('No Harbor profile is active');
    expect(() => useFeedStore.getState().toggleSave('post-a')).toThrow(
      'No Harbor profile is active',
    );
  });

  it('restores isolated profile values across an A to B to A switch', async () => {
    await switchProfile('profile-a');
    useSettingsStore.getState().addBootstrapNode('/dns4/a.example/tcp/443');
    useSettingsStore.getState().setDefaultVisibility('public');
    useSettingsStore.getState().setSocialView('videos');
    useSettingsStore.getState().setCommunityView('audio');
    useSettingsStore.getState().setProviderEmbedConsent('session');
    useSettingsStore.getState().setTheme('dark');
    useMessagingStore.getState().archiveConversation('peer-a');
    useNotificationsStore.getState().setNativeEnabled(true);
    useFeedStore.getState().toggleSave('post-a');
    useFeedStore.getState().hidePost('hidden-a');
    useFeedStore.getState().snoozeAuthor('author-a', 1);

    await switchProfile('profile-b');
    expect(useSettingsStore.getState()).toMatchObject({
      bootstrapNodes: [],
      defaultVisibility: 'contacts',
      socialView: 'all',
      communityView: 'all',
      providerEmbedConsent: 'per-use',
      theme: 'dark',
    });
    expect(useMessagingStore.getState().archivedConversations).toEqual([]);
    expect(useNotificationsStore.getState().nativeEnabled).toBe(false);
    expect(useFeedStore.getState()).toMatchObject({
      savedPostIds: [],
      hiddenPostIds: [],
      snoozedAuthors: [],
    });

    useMessagingStore.getState().archiveConversation('peer-b');
    useFeedStore.getState().toggleSave('post-b');

    await switchProfile('profile-a');
    expect(useSettingsStore.getState()).toMatchObject({
      bootstrapNodes: ['/dns4/a.example/tcp/443'],
      defaultVisibility: 'public',
      socialView: 'videos',
      communityView: 'audio',
      providerEmbedConsent: 'session',
      theme: 'dark',
    });
    expect(useMessagingStore.getState().archivedConversations).toEqual(['peer-a']);
    expect(useNotificationsStore.getState().nativeEnabled).toBe(true);
    expect(useFeedStore.getState().savedPostIds).toEqual(['post-a']);
    expect(useFeedStore.getState().hiddenPostIds).toEqual(['hidden-a']);
    expect(useFeedStore.getState().snoozedAuthors[0].peerId).toBe('author-a');
  });

  it('moves legacy profile settings once and preserves device appearance', async () => {
    localStorage.setItem(
      'harbor-settings',
      JSON.stringify({
        state: {
          avatarUrl: 'legacy-avatar',
          socialView: 'images',
          theme: 'dark',
          accentColor: 'purple',
          fontSize: 'large',
        },
        version: 1,
      }),
    );

    await switchProfile('profile-a');
    expect(useSettingsStore.getState()).toMatchObject({
      socialView: 'images',
      theme: 'dark',
      accentColor: 'purple',
      fontSize: 'large',
    });
    expect(localStorage.getItem('harbor-settings')).toBeNull();
    expect(localStorage.getItem('harbor:profile:profile-a:settings:v3')).not.toBeNull();
    expect(JSON.parse(localStorage.getItem('harbor-device-appearance-v1') ?? '{}').state).toEqual({
      theme: 'dark',
      accentColor: 'purple',
      fontSize: 'large',
    });

    await switchProfile('profile-b');
    expect('avatarUrl' in useSettingsStore.getState()).toBe(false);
    expect(useSettingsStore.getState().socialView).toBe('all');
    expect(useSettingsStore.getState().theme).toBe('dark');
  });

  it('upgrades profile settings without retaining the retired data-url avatar fallback', async () => {
    localStorage.setItem(
      'harbor:profile:profile-a:settings:v2',
      JSON.stringify({
        state: { socialView: 'videos', avatarUrl: 'data:image/png;base64,legacy' },
        version: 2,
      }),
    );

    await switchProfile('profile-a');

    expect(useSettingsStore.getState().socialView).toBe('videos');
    expect('avatarUrl' in useSettingsStore.getState()).toBe(false);
    expect(localStorage.getItem('harbor:profile:profile-a:settings:v2')).toBeNull();
    expect(localStorage.getItem('harbor:profile:profile-a:settings:v3')).not.toContain('avatarUrl');
  });

  it('clears all profile-owned runtime data during suspension', () => {
    useMessagingStore.setState({
      conversations: [{ conversationId: 'conversation-a' } as never],
      messages: { 'peer-a': [{ messageId: 'message-a' } as never] },
      activeConversation: 'peer-a',
      selectedConversationId: 'conversation-a',
      isLoading: true,
      error: 'profile-a-error',
    });
    useFeedStore.setState({
      rawFeedItems: [{ postId: 'post-a' } as never],
      feedItems: [{ postId: 'post-a' } as never],
      comments: { 'post-a': [{ commentId: 'comment-a' } as never] },
      commentCounts: { 'post-a': 1 },
      expandedComments: new Set(['post-a']),
      loadingComments: new Set(['post-a']),
      lastSyncAt: 123,
    });

    resetProfilePersistenceMemory();

    expect(useMessagingStore.getState()).toMatchObject({
      conversations: [],
      messages: {},
      activeConversation: null,
      selectedConversationId: null,
      isLoading: false,
      error: null,
    });
    expect(useFeedStore.getState()).toMatchObject({
      rawFeedItems: [],
      feedItems: [],
      comments: {},
      commentCounts: {},
      lastSyncAt: null,
    });
    expect(useFeedStore.getState().expandedComments.size).toBe(0);
    expect(useFeedStore.getState().loadingComments.size).toBe(0);
  });
});
