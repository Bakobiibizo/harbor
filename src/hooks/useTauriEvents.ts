import { useEffect, useRef } from 'react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import toast from 'react-hot-toast';
import type { NetworkEvent } from '../types';
import { useNetworkStore, useContactsStore, useMessagingStore, useFeedStore, useBoardsStore } from '../stores';
import { playMessageSound, playPostSound, playCommunitySound } from '../services/audioNotifications';
import { createLogger } from '../utils/logger';

const log = createLogger('TauriEvents');
const netLog = createLogger('Network');

/**
 * Hook to listen to Tauri events from the Rust backend.
 * Should be called once at the app root level.
 */
export function useTauriEvents() {
  const unlistenersRef = useRef<UnlistenFn[]>([]);
  const { refreshPeers, refreshStats } = useNetworkStore();
  const { refreshContacts } = useContactsStore();

  useEffect(() => {
    async function setupListeners() {
      // Listen to network events
      const unlistenNetwork = await listen<NetworkEvent>('harbor:network', (event) => {
        try {
          log.info('harbor:network:', event.payload);
          handleNetworkEvent(event.payload);
        } catch (err) {
          log.error('Error in harbor:network event listener', err);
        }
      });
      unlistenersRef.current.push(unlistenNetwork);

      // Future: Listen to message events
      // const unlistenMessage = await listen<MessageEvent>(
      //   "harbor:message",
      //   (event) => handleMessageEvent(event.payload)
      // );
      // unlistenersRef.current.push(unlistenMessage);
    }

    function handleNetworkEvent(event: NetworkEvent) {
      try {
        switch (event.type) {
          case 'peer_connected':
            try {
              netLog.info(`Peer connected: ${event.peerId}`);
              // Refresh the full peer list to get updated info
              refreshPeers().catch((err) =>
                netLog.error('Failed to refresh peers on peer_connected', err),
              );
              refreshStats().catch((err) =>
                netLog.error('Failed to refresh stats on peer_connected', err),
              );
            } catch (err) {
              netLog.error('Error handling peer_connected event', err);
            }
            break;

          case 'peer_disconnected':
            try {
              netLog.info(`Peer disconnected: ${event.peerId}`);
              refreshPeers().catch((err) =>
                netLog.error('Failed to refresh peers on peer_disconnected', err),
              );
              refreshStats().catch((err) =>
                netLog.error('Failed to refresh stats on peer_disconnected', err),
              );
            } catch (err) {
              netLog.error('Error handling peer_disconnected event', err);
            }
            break;

          case 'peer_discovered':
            try {
              netLog.info(`Peer discovered: ${event.peerId}`);
              refreshPeers().catch((err) =>
                netLog.error('Failed to refresh peers on peer_discovered', err),
              );
            } catch (err) {
              netLog.error('Error handling peer_discovered event', err);
            }
            break;

          case 'peer_expired':
            try {
              netLog.info(`Peer expired: ${event.peerId}`);
              refreshPeers().catch((err) =>
                netLog.error('Failed to refresh peers on peer_expired', err),
              );
            } catch (err) {
              netLog.error('Error handling peer_expired event', err);
            }
            break;

          case 'message_received':
            try {
              netLog.info(`Message received from ${event.peerId} via ${event.protocol}`);
              // Play notification sound (smart muting checks active conversation)
              playMessageSound(event.peerId);
              // Use getState() to avoid stale closures - call functions directly from the store
              const messagingState = useMessagingStore.getState();
              // Always refresh conversations to update previews and unread counts
              messagingState
                .loadConversations()
                .catch((err) =>
                  netLog.error('Failed to load conversations on message_received', err),
                );
              // Reload messages if we're viewing the sender's conversation
              const activeConv = messagingState.activeConversation;
              netLog.debug(
                `Active conversation: ${activeConv}, message from: ${event.peerId}`,
              );
              if (activeConv === event.peerId) {
                netLog.info(`Reloading messages for active conversation: ${activeConv}`);
                messagingState
                  .loadMessages(activeConv)
                  .catch((err) =>
                    netLog.error('Failed to reload messages on message_received', err),
                  );
              }
              // Also refresh contacts in case this is from a new contact
              refreshContacts().catch((err) =>
                netLog.error('Failed to refresh contacts on message_received', err),
              );
            } catch (err) {
              netLog.error('Error handling message_received event', err);
            }
            break;

          case 'listening_on':
            try {
              netLog.info(`Listening on: ${event.address}`);
            } catch (err) {
              netLog.error('Error handling listening_on event', err);
            }
            break;

          case 'external_address_discovered':
            try {
              netLog.info(`External address: ${event.address}`);
            } catch (err) {
              netLog.error('Error handling external_address_discovered event', err);
            }
            break;

          case 'status_changed':
            try {
              netLog.info(`Status changed: ${event.status}`);
            } catch (err) {
              netLog.error('Error handling status_changed event', err);
            }
            break;

          case 'contact_added':
            try {
              netLog.info(`Contact added: ${event.displayName} (${event.peerId})`);
              refreshContacts().catch((err) =>
                netLog.error('Failed to refresh contacts on contact_added', err),
              );
              toast.success(`Added ${event.displayName} to contacts!`);
            } catch (err) {
              netLog.error('Error handling contact_added event', err);
            }
            break;

          case 'nat_status_changed':
            try {
              netLog.info(`NAT status changed: ${event.status}`);
              // Update NAT status in store
              useNetworkStore.getState().setNatStatus(event.status);
              // Show toast for important status changes
              if (event.status === 'public') {
                toast.success('Public IP detected - direct connections possible');
              } else if (event.status === 'private') {
                toast('Behind NAT - using relay for remote connections', { icon: '🔄' });
              }
            } catch (err) {
              netLog.error('Error handling nat_status_changed event', err);
            }
            break;

          case 'relay_connected':
            try {
              netLog.info(`Relay connected: ${event.relayAddress}`);
              // Dismiss any pending timeout/warning toasts
              toast.dismiss();
              // Add relay address to store
              useNetworkStore.getState().addRelayAddress(event.relayAddress);
              // Update relay status
              useNetworkStore.getState().setRelayStatus('connected');
              // Refresh addresses to update the UI
              useNetworkStore
                .getState()
                .refreshAddresses()
                .catch((err) =>
                  netLog.error('Failed to refresh addresses on relay_connected', err),
                );
              useNetworkStore
                .getState()
                .refreshShareableAddresses()
                .catch((err) =>
                  netLog.error('Failed to refresh shareable addresses on relay_connected', err),
                );
              toast.success('Connected to Harbor relay');
            } catch (err) {
              netLog.error('Error handling relay_connected event', err);
            }
            break;

          case 'hole_punch_succeeded':
            try {
              netLog.info(`Hole punch succeeded with: ${event.peerId}`);
              toast.success('Direct connection established!');
            } catch (err) {
              netLog.error('Error handling hole_punch_succeeded event', err);
            }
            break;

          case 'content_manifest_received':
            try {
              netLog.info(
                `Content manifest received from ${event.peerId}: ${event.postCount} posts, hasMore: ${event.hasMore}`,
              );
            } catch (err) {
              netLog.error('Error handling content_manifest_received event', err);
            }
            break;

          case 'content_fetched':
            try {
              netLog.info(`Content fetched from ${event.peerId}: post ${event.postId}`);
              // Play notification sound for new wall post (smart muting checks feed view)
              playPostSound();
              // Refresh the feed to show new posts
              useFeedStore
                .getState()
                .loadFeed()
                .catch((err) =>
                  netLog.error('Failed to load feed on content_fetched', err),
                );
            } catch (err) {
              netLog.error('Error handling content_fetched event', err);
            }
            break;

          case 'content_sync_error':
            try {
              netLog.warn(`Content sync error from ${event.peerId}: ${event.error}`);
            } catch (err) {
              netLog.error('Error handling content_sync_error event', err);
            }
            break;

          case 'board_list_received':
            try {
              netLog.info(
                `Board list received from ${event.relayPeerId}: ${event.boardCount} boards`,
              );
              // Refresh communities and boards in the store
              const boardsStateList = useBoardsStore.getState();
              boardsStateList.loadCommunities().catch((err) =>
                netLog.error('Failed to reload communities on board_list_received', err),
              );
              // If this relay is the active community, refresh its boards
              if (boardsStateList.activeCommunity?.relayPeerId === event.relayPeerId) {
                boardsStateList.selectCommunity(boardsStateList.activeCommunity).catch((err) =>
                  netLog.error('Failed to refresh boards on board_list_received', err),
                );
              }
            } catch (err) {
              netLog.error('Error handling board_list_received event', err);
            }
            break;

          case 'board_posts_received':
            try {
              netLog.info(
                `Board posts received from ${event.relayPeerId}: board ${event.boardId}, ${event.postCount} posts`,
              );
              // Play community notification sound (smart muting checks active board)
              playCommunitySound(event.boardId);
              // Refresh the board if we're currently viewing it
              const boardsStatePosts = useBoardsStore.getState();
              if (boardsStatePosts.activeBoard?.boardId === event.boardId) {
                boardsStatePosts.loadBoardPosts().catch((err) =>
                  netLog.error('Failed to reload board posts on board_posts_received', err),
                );
              }
            } catch (err) {
              netLog.error('Error handling board_posts_received event', err);
            }
            break;

          case 'board_post_submitted':
            try {
              netLog.info(
                `Board post submitted to ${event.relayPeerId}: post ${event.postId}`,
              );
            } catch (err) {
              netLog.error('Error handling board_post_submitted event', err);
            }
            break;

          case 'board_sync_error':
            try {
              netLog.warn(
                `Board sync error from ${event.relayPeerId}: ${event.error}`,
              );
              toast.error(`Board sync error: ${event.error}`);
            } catch (err) {
              netLog.error('Error handling board_sync_error event', err);
            }
            break;

          case 'community_auto_joined':
            try {
              netLog.info(
                `Auto-joined community on ${event.relayPeerId}: ${event.boardCount} boards`,
              );
              playCommunitySound();
              // Mark this relay as a community relay in the network store
              useNetworkStore.getState().addCommunityRelay(event.relayPeerId);
              // Refresh communities list to show the new community
              useBoardsStore.getState().loadCommunities().catch((err) =>
                netLog.error('Failed to reload communities on community_auto_joined', err),
              );
              toast.success(
                `Joined ${event.communityName || 'community'} (${event.boardCount} boards)`,
              );
            } catch (err) {
              netLog.error('Error handling community_auto_joined event', err);
            }
            break;

          case 'message_ack_received':
            try {
              netLog.info(
                `Message ack received: ${event.messageId} is now ${event.status}`,
              );
              const ackMessagingState = useMessagingStore.getState();
              // Update the message status in the local store
              if (event.status === 'delivered') {
                ackMessagingState.updateMessageStatus(
                  event.messageId,
                  'delivered',
                  event.timestamp,
                  undefined,
                );
              } else if (event.status === 'read') {
                ackMessagingState.updateMessageStatus(
                  event.messageId,
                  'read',
                  undefined,
                  event.timestamp,
                );
              }
              // Also refresh conversations to update any previews
              ackMessagingState
                .loadConversations()
                .catch((err) =>
                  netLog.error('Failed to load conversations on message_ack_received', err),
                );
            } catch (err) {
              netLog.error('Error handling message_ack_received event', err);
            }
            break;

          case 'wall_post_synced':
            try {
              netLog.info(`Wall post synced to relay: ${event.postId}`);
            } catch (err) {
              netLog.error('Error handling wall_post_synced event', err);
            }
            break;

          case 'wall_posts_received':
            try {
              netLog.info(
                `Wall posts received from relay: ${event.postCount} posts from ${event.authorPeerId}`,
              );
              // Refresh the feed to show new posts from the relay
              useFeedStore
                .getState()
                .loadFeed()
                .catch((err) =>
                  netLog.error('Failed to load feed on wall_posts_received', err),
                );
            } catch (err) {
              netLog.error('Error handling wall_posts_received event', err);
            }
            break;
        }
      } catch (err) {
        netLog.error('Unhandled error in network event handler', err);
      }
    }

    setupListeners().catch((err) => log.error('Failed to set up event listeners', err));

    // Cleanup on unmount
    return () => {
      unlistenersRef.current.forEach((unlisten) => unlisten());
      unlistenersRef.current = [];
    };
  }, [refreshPeers, refreshStats, refreshContacts]);
}
