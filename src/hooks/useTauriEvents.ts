import { useEffect, useRef } from 'react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import toast from 'react-hot-toast';
import type { NetworkEvent } from '../types';
import { useNetworkStore, useContactsStore, useMessagingStore, useFeedStore } from '../stores';

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
        console.log('[TauriEvent] harbor:network:', event.payload);
        handleNetworkEvent(event.payload);
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
              console.log(`[Network] Peer connected: ${event.peerId}`);
              // Refresh the full peer list to get updated info
              refreshPeers().catch((err) =>
                console.error('[Network] Failed to refresh peers on peer_connected:', err),
              );
              refreshStats().catch((err) =>
                console.error('[Network] Failed to refresh stats on peer_connected:', err),
              );
            } catch (err) {
              console.error('[Network] Error handling peer_connected event:', err);
            }
            break;

          case 'peer_disconnected':
            try {
              console.log(`[Network] Peer disconnected: ${event.peerId}`);
              refreshPeers().catch((err) =>
                console.error('[Network] Failed to refresh peers on peer_disconnected:', err),
              );
              refreshStats().catch((err) =>
                console.error('[Network] Failed to refresh stats on peer_disconnected:', err),
              );
            } catch (err) {
              console.error('[Network] Error handling peer_disconnected event:', err);
            }
            break;

          case 'peer_discovered':
            try {
              console.log(`[Network] Peer discovered: ${event.peerId}`);
              refreshPeers().catch((err) =>
                console.error('[Network] Failed to refresh peers on peer_discovered:', err),
              );
            } catch (err) {
              console.error('[Network] Error handling peer_discovered event:', err);
            }
            break;

          case 'peer_expired':
            try {
              console.log(`[Network] Peer expired: ${event.peerId}`);
              refreshPeers().catch((err) =>
                console.error('[Network] Failed to refresh peers on peer_expired:', err),
              );
            } catch (err) {
              console.error('[Network] Error handling peer_expired event:', err);
            }
            break;

          case 'message_received':
            try {
              console.log(`[Network] Message received from ${event.peerId} via ${event.protocol}`);
              // Use getState() to avoid stale closures - call functions directly from the store
              const messagingState = useMessagingStore.getState();
              // Always refresh conversations to update previews and unread counts
              messagingState
                .loadConversations()
                .catch((err) =>
                  console.error('[Network] Failed to load conversations on message_received:', err),
                );
              // Reload messages if we're viewing the sender's conversation
              const activeConv = messagingState.activeConversation;
              console.log(
                `[Network] Active conversation: ${activeConv}, message from: ${event.peerId}`,
              );
              if (activeConv === event.peerId) {
                console.log(`[Network] Reloading messages for active conversation: ${activeConv}`);
                messagingState
                  .loadMessages(activeConv)
                  .catch((err) =>
                    console.error('[Network] Failed to reload messages on message_received:', err),
                  );
              }
              // Also refresh contacts in case this is from a new contact
              refreshContacts().catch((err) =>
                console.error('[Network] Failed to refresh contacts on message_received:', err),
              );
            } catch (err) {
              console.error('[Network] Error handling message_received event:', err);
            }
            break;

          case 'listening_on':
            try {
              console.log(`[Network] Listening on: ${event.address}`);
            } catch (err) {
              console.error('[Network] Error handling listening_on event:', err);
            }
            break;

          case 'external_address_discovered':
            try {
              console.log(`[Network] External address: ${event.address}`);
            } catch (err) {
              console.error('[Network] Error handling external_address_discovered event:', err);
            }
            break;

          case 'status_changed':
            try {
              console.log(`[Network] Status changed: ${event.status}`);
            } catch (err) {
              console.error('[Network] Error handling status_changed event:', err);
            }
            break;

          case 'contact_added':
            try {
              console.log(`[Network] Contact added: ${event.displayName} (${event.peerId})`);
              refreshContacts().catch((err) =>
                console.error('[Network] Failed to refresh contacts on contact_added:', err),
              );
              toast.success(`Added ${event.displayName} to contacts!`);
            } catch (err) {
              console.error('[Network] Error handling contact_added event:', err);
            }
            break;

          case 'nat_status_changed':
            try {
              console.log(`[Network] NAT status changed: ${event.status}`);
              // Update NAT status in store
              useNetworkStore.getState().setNatStatus(event.status);
              // Show toast for important status changes
              if (event.status === 'public') {
                toast.success('Public IP detected - direct connections possible');
              } else if (event.status === 'private') {
                toast('Behind NAT - using relay for remote connections', { icon: '🔄' });
              }
            } catch (err) {
              console.error('[Network] Error handling nat_status_changed event:', err);
            }
            break;

          case 'relay_connected':
            try {
              console.log(`[Network] Relay connected: ${event.relayAddress}`);
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
                  console.error('[Network] Failed to refresh addresses on relay_connected:', err),
                );
              useNetworkStore
                .getState()
                .refreshShareableAddresses()
                .catch((err) =>
                  console.error(
                    '[Network] Failed to refresh shareable addresses on relay_connected:',
                    err,
                  ),
                );
              toast.success('Connected to Harbor relay');
            } catch (err) {
              console.error('[Network] Error handling relay_connected event:', err);
            }
            break;

          case 'hole_punch_succeeded':
            try {
              console.log(`[Network] Hole punch succeeded with: ${event.peerId}`);
              toast.success('Direct connection established!');
            } catch (err) {
              console.error('[Network] Error handling hole_punch_succeeded event:', err);
            }
            break;

          case 'content_manifest_received':
            try {
              console.log(
                `[Network] Content manifest received from ${event.peerId}: ${event.postCount} posts, hasMore: ${event.hasMore}`,
              );
            } catch (err) {
              console.error('[Network] Error handling content_manifest_received event:', err);
            }
            break;

          case 'content_fetched':
            try {
              console.log(`[Network] Content fetched from ${event.peerId}: post ${event.postId}`);
              // Refresh the feed to show new posts
              useFeedStore
                .getState()
                .loadFeed()
                .catch((err) =>
                  console.error('[Network] Failed to load feed on content_fetched:', err),
                );
            } catch (err) {
              console.error('[Network] Error handling content_fetched event:', err);
            }
            break;

          case 'content_sync_error':
            try {
              console.warn(`[Network] Content sync error from ${event.peerId}: ${event.error}`);
            } catch (err) {
              console.error('[Network] Error handling content_sync_error event:', err);
            }
            break;
        }
      } catch (err) {
        console.error('[Network] Unhandled error in network event handler:', err);
      }
    }

    setupListeners();

    // Cleanup on unmount
    return () => {
      unlistenersRef.current.forEach((unlisten) => unlisten());
      unlistenersRef.current = [];
    };
  }, [refreshPeers, refreshStats, refreshContacts]);
}
