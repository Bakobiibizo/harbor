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
      switch (event.type) {
        case 'peer_connected':
          console.log(`[Network] Peer connected: ${event.peerId}`);
          // Refresh the full peer list to get updated info
          refreshPeers();
          refreshStats();
          break;

        case 'peer_disconnected':
          console.log(`[Network] Peer disconnected: ${event.peerId}`);
          refreshPeers();
          refreshStats();
          break;

        case 'peer_discovered':
          console.log(`[Network] Peer discovered: ${event.peerId}`);
          refreshPeers();
          break;

        case 'peer_expired':
          console.log(`[Network] Peer expired: ${event.peerId}`);
          refreshPeers();
          break;

        case 'message_received':
          console.log(`[Network] Message received from ${event.peerId} via ${event.protocol}`);
          // Use getState() to avoid stale closures - call functions directly from the store
          const messagingState = useMessagingStore.getState();
          // Always refresh conversations to update previews and unread counts
          messagingState.loadConversations();
          // Reload messages if we're viewing the sender's conversation
          const activeConv = messagingState.activeConversation;
          console.log(
            `[Network] Active conversation: ${activeConv}, message from: ${event.peerId}`,
          );
          if (activeConv === event.peerId) {
            console.log(`[Network] Reloading messages for active conversation: ${activeConv}`);
            messagingState.loadMessages(activeConv);
          }
          // Also refresh contacts in case this is from a new contact
          refreshContacts();
          break;

        case 'listening_on':
          console.log(`[Network] Listening on: ${event.address}`);
          break;

        case 'external_address_discovered':
          console.log(`[Network] External address: ${event.address}`);
          break;

        case 'status_changed':
          console.log(`[Network] Status changed: ${event.status}`);
          break;

        case 'contact_added':
          console.log(`[Network] Contact added: ${event.displayName} (${event.peerId})`);
          refreshContacts();
          toast.success(`Added ${event.displayName} to contacts!`);
          break;

        case 'nat_status_changed':
          console.log(`[Network] NAT status changed: ${event.status}`);
          // Update NAT status in store
          useNetworkStore.getState().setNatStatus(event.status);
          // Show toast for important status changes
          if (event.status === 'public') {
            toast.success('Public IP detected - direct connections possible');
          } else if (event.status === 'private') {
            toast('Behind NAT - using relay for remote connections', { icon: '🔄' });
          }
          break;

        case 'relay_connected':
          console.log(`[Network] Relay connected: ${event.relayAddress}`);
          // Dismiss any pending timeout/warning toasts
          toast.dismiss();
          // Add relay address to store
          useNetworkStore.getState().addRelayAddress(event.relayAddress);
          // Update relay status
          useNetworkStore.getState().setRelayStatus('connected');
          // Refresh addresses to update the UI
          useNetworkStore.getState().refreshAddresses();
          useNetworkStore.getState().refreshShareableAddresses();
          toast.success('Connected to Harbor relay');
          break;

        case 'hole_punch_succeeded':
          console.log(`[Network] Hole punch succeeded with: ${event.peerId}`);
          toast.success('Direct connection established!');
          break;

        case 'content_manifest_received':
          console.log(
            `[Network] Content manifest received from ${event.peerId}: ${event.postCount} posts, hasMore: ${event.hasMore}`,
          );
          break;

        case 'content_fetched':
          console.log(`[Network] Content fetched from ${event.peerId}: post ${event.postId}`);
          // Refresh the feed to show new posts
          useFeedStore.getState().loadFeed();
          break;

        case 'content_sync_error':
          console.warn(`[Network] Content sync error from ${event.peerId}: ${event.error}`);
          break;
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
