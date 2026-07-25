import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import toast from 'react-hot-toast';
import type { NetworkEvent } from '../types';
import {
  useNetworkStore,
  useContactsStore,
  useMessagingStore,
  useFeedStore,
  useContactWallStore,
  useWallStore,
  useCallingStore,
  useIdentityStore,
  useMediaTransfersStore,
  useSettingsStore,
} from '../stores';
import { mediaService } from '../services/media';
import { feedService } from '../services/feed';
import { ReactiveRefreshCoordinator } from '../services/reactiveRefresh';
import { ContactFeedPoller } from '../services/contactFeedPoller';
import { notifyHarborEvent } from '../services/harborNotifications';
import { isMediaTransferEventForIdentity } from '../services/mediaTransferEvents';
import { safePeerLabel } from '../utils/relayName';
import { summarizeSignalingForLog } from '../utils/signalingLog';
import { registerProfileRuntimeReset } from '../services/profileRuntimeLifecycle';
import { applyPostRelayStatusEvent } from '../stores/wall';
import { createAsyncDisposerScope, registerAtomicResources } from '../utils/asyncDisposer';
import type { ProfileToken } from '../services/profileSession';
import {
  beginProfileEventRegistration,
  clearProfileEventRegistration,
  markProfileEventsReady,
} from '../services/profileEventReadiness';

/**
 * Hook to listen to Tauri events from the Rust backend.
 * Should be called once at the app root level.
 */
export function useTauriEvents(profileToken: ProfileToken) {
  const { refreshPeers, refreshStats } = useNetworkStore();

  useEffect(() => {
    let listenersReady = false;
    const reportListenerError = (error: unknown) => {
      console.warn('[TauriEvent] Listener lifecycle failure:', error);
    };
    const listenerScope = createAsyncDisposerScope(reportListenerError);
    const readinessLease = beginProfileEventRegistration(profileToken);
    const coordinator = new ReactiveRefreshCoordinator({
      contacts: () => useContactsStore.getState().refreshContacts(),
      requests: () => useContactsStore.getState().loadRequests(),
      messages: async (peerIds) => {
        const messaging = useMessagingStore.getState();
        const activePeer = messaging.activeConversation;
        await messaging.loadConversations();
        if (activePeer && (peerIds.size === 0 || peerIds.has(activePeer))) {
          await messaging.loadMessages(activePeer);
        }
      },
      posts: async (peerIds) => {
        await useFeedStore.getState().loadFeed();
        const contactWall = useContactWallStore.getState();
        if (
          contactWall.authorPeerId &&
          (peerIds.size === 0 || peerIds.has(contactWall.authorPeerId))
        ) {
          await contactWall.reconcileWall();
        }
      },
      media: () => mediaService.preloadMissingMedia(),
    });
    coordinator.start();

    const contactFeedPoller = new ContactFeedPoller({
      fetchContact: (peerId) => feedService.fetchContactWall(peerId),
      publishRefresh: (peerId) => coordinator.enqueue({ domains: ['posts', 'media'], peerId }),
    });
    let activeMediaProfileId: string | null = null;
    const reconcileContactFeedPoller = () => {
      const identity = useIdentityStore.getState().state;
      const network = useNetworkStore.getState();
      const contacts = useContactsStore.getState();
      const settings = useSettingsStore.getState();
      const profileId = identity.status === 'unlocked' ? identity.identity.peerId : null;
      if (profileId) coordinator.start();
      else coordinator.stop();
      if (profileId !== activeMediaProfileId) {
        activeMediaProfileId = profileId;
        useMediaTransfersStore.getState().reset();
      }
      contactFeedPoller.update({
        profileId,
        online:
          network.isRunning &&
          network.status === 'connected' &&
          (typeof navigator === 'undefined' || navigator.onLine),
        enabled: settings.contactFeedPollingEnabled,
        intervalMs: settings.contactFeedPollIntervalMinutes * 60_000,
        contacts: contacts.contacts,
        requests: contacts.requests,
      });
    };
    const storeUnsubscribers = [
      useIdentityStore.subscribe(reconcileContactFeedPoller),
      useNetworkStore.subscribe(reconcileContactFeedPoller),
      useContactsStore.subscribe(reconcileContactFeedPoller),
      useSettingsStore.subscribe(reconcileContactFeedPoller),
    ];
    window.addEventListener('online', reconcileContactFeedPoller);
    window.addEventListener('offline', reconcileContactFeedPoller);
    reconcileContactFeedPoller();
    const unregisterProfileReset = registerProfileRuntimeReset(() => {
      activeMediaProfileId = null;
      contactFeedPoller.stop();
      coordinator.stop();
    });

    async function setupListeners() {
      const registered = await registerAtomicResources(
        listenerScope,
        [
          // Listen to network events
          () =>
            listen<NetworkEvent>('harbor:network', (event) => {
              if (!listenersReady || listenerScope.disposed) return;
              console.log(
                '[TauriEvent] harbor:network:',
                event.payload.type === 'call_signaling_received'
                  ? summarizeSignalingForLog(event.payload.message, 'inbound', 'received')
                  : event.payload,
              );
              handleNetworkEvent(event.payload);
            }),
          // Listen for deep-link contact strings forwarded from the OS via Rust
          () =>
            listen<string>('deep_link_contact', (event) => {
              if (!listenersReady || listenerScope.disposed) return;
              useNetworkStore.getState().setPendingDeepLinkContact(event.payload);
            }),
        ],
        reportListenerError,
      );
      listenersReady = registered && !listenerScope.disposed;
      if (listenersReady && !markProfileEventsReady(readinessLease)) {
        listenersReady = false;
        listenerScope.dispose();
      }

      // Future: Listen to message events
      // const unlistenMessage = await listen<MessageEvent>(
      //   "harbor:message",
      //   (event) => handleMessageEvent(event.payload)
      // );
      // Add this registration to the atomic listener group above when enabled.
    }

    function handleNetworkEvent(event: NetworkEvent) {
      if (useIdentityStore.getState().state.status !== 'unlocked') return;
      const contactName = (peerId: string) => {
        const contact = useContactsStore.getState().contacts.find((item) => item.peerId === peerId);
        return safePeerLabel(peerId, contact?.verifiedQualifiedName, contact?.displayName);
      };
      switch (event.type) {
        case 'peer_connected':
          console.log(`[Network] Peer connected: ${event.peer_id}`);
          // Refresh the full peer list to get updated info
          refreshPeers();
          refreshStats();
          // Trigger media preloader — a newly connected peer may be an author
          // whose images we need to fetch (e.g. after relay circuit dial)
          coordinator.enqueue({ domains: ['media'], peerId: event.peer_id });
          break;

        case 'peer_disconnected':
          console.log(`[Network] Peer disconnected: ${event.peer_id}`);
          useCallingStore.getState().handlePeerDisconnected(event.peer_id);
          refreshPeers();
          refreshStats();
          break;

        case 'peer_discovered':
          console.log(`[Network] Peer discovered: ${event.peer_id}`);
          refreshPeers();
          break;

        case 'peer_expired':
          console.log(`[Network] Peer expired: ${event.peer_id}`);
          refreshPeers();
          break;

        case 'message_received':
          console.log(`[Network] Message received from ${event.peer_id} via ${event.protocol}`);
          notifyHarborEvent({
            kind: 'message',
            peerId: event.peer_id,
            senderName: contactName(event.peer_id),
            eventId: `${event.protocol}:${event.payload.slice(0, 16).join('.')}`,
          });
          // Use getState() to avoid stale closures - call functions directly from the store
          coordinator.enqueue({
            domains: ['messages', 'contacts'],
            peerId: event.peer_id,
          });
          break;

        case 'message_delivery_changed':
          useMessagingStore
            .getState()
            .updateMessageStatus(
              event.message_id,
              event.status,
              event.status === 'delivered' ? event.timestamp : undefined,
              event.status === 'read' ? event.timestamp : undefined,
            );
          if (event.status === 'failed' && event.error) {
            toast.error(event.error);
          }
          break;

        case 'message_ack_received':
          useMessagingStore
            .getState()
            .updateMessageStatus(
              event.message_id,
              event.status,
              event.status === 'delivered' ? event.timestamp : undefined,
              event.status === 'read' ? event.timestamp : undefined,
            );
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
          console.log(`[Network] Contact added: ${event.display_name} (${event.peer_id})`);
          coordinator.enqueue({
            domains: ['contacts', 'requests', 'posts'],
            peerId: event.peer_id,
          });
          toast.success('Contact added. Harbor is verifying their relay name.');
          break;

        case 'contact_request_changed':
          coordinator.enqueue({ domains: ['requests'], peerId: event.peer_id });
          if (event.direction === 'incoming' && event.status === 'review') {
            toast(
              `Contact request from ${safePeerLabel(event.peer_id, undefined, event.display_name)}`,
              {
                icon: '👤',
                duration: 6000,
              },
            );
          } else if (event.status === 'accepted') {
            coordinator.enqueue({ domains: ['contacts', 'posts'], peerId: event.peer_id });
            toast.success('Contact request accepted');
          } else if (event.status === 'declined') {
            toast('Contact request declined');
          } else if (event.status === 'failed') {
            toast.error('Contact request could not be delivered');
          }
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
          console.log(`[Network] Relay connected: ${event.relay_address}`);
          // Dismiss any pending timeout/warning toasts
          toast.dismiss();
          // Add relay address to store
          useNetworkStore.getState().addRelayAddress(event.relay_address);
          // Update relay status
          useNetworkStore.getState().setRelayStatus('connected');
          // Refresh addresses to update the UI
          useNetworkStore.getState().refreshAddresses();
          useNetworkStore.getState().refreshShareableAddresses();
          toast.success('Connected to Harbor relay');
          break;

        case 'hole_punch_succeeded':
          console.log(`[Network] Hole punch succeeded with: ${event.peer_id}`);
          toast.success('Direct connection established!');
          break;

        case 'content_manifest_received':
          console.log(
            `[Network] Content manifest received from ${event.peer_id}: ${event.post_count} posts, hasMore: ${event.has_more}`,
          );
          break;

        case 'content_fetched':
          console.log(`[Network] Content fetched from ${event.peer_id}: post ${event.post_id}`);
          // Refresh the feed to show new posts
          coordinator.enqueue({ domains: ['posts', 'contacts'], peerId: event.peer_id });
          break;

        case 'content_sync_error':
          console.warn(`[Network] Content sync error from ${event.peer_id}: ${event.error}`);
          break;

        case 'wall_sync_status': {
          console.log(`[Network] Wall sync ${event.scope}/${event.phase}: ${event.status}`);
          const status =
            event.status === 'success' || event.status === 'partial_failure'
              ? event.status
              : 'in_progress';
          const patch = {
            lastSyncAt: event.occurred_at,
            syncStatus: status,
            syncError: event.error,
          } as const;
          if (event.scope === 'author_wall') {
            useWallStore.setState({
              ...patch,
              isSyncingRelay: status === 'in_progress',
            });
          } else if (event.scope === 'contact_wall') {
            useContactWallStore.setState({
              ...patch,
              isSyncing: status === 'in_progress',
            });
          } else if (event.scope === 'feed') {
            useFeedStore.setState({
              ...patch,
              isSyncingRelay: status === 'in_progress',
            });
          }
          if (event.status === 'success' || event.status === 'partial_failure') {
            coordinator.enqueue({
              domains: ['posts'],
              peerId: event.author_peer_id ?? undefined,
            });
          }
          break;
        }

        case 'wall_post_synced':
          console.log(`[Network] Wall post synced to relay: ${event.post_id}`);
          break;

        case 'post_relay_status_changed':
          applyPostRelayStatusEvent(event);
          coordinator.enqueue({ domains: ['posts'] });
          break;

        case 'wall_posts_received':
          console.log(
            `[Network] Wall posts received from relay (author: ${event.author_peer_id}, count: ${event.post_count})`,
          );
          // Reload feed to show newly received posts
          coordinator.enqueue({
            domains: ['posts', 'media'],
            peerId: event.author_peer_id,
          });
          break;

        case 'media_fetched':
          console.log(`[Network] Attachment received from ${event.peer_id}`);
          // Refresh feed to display newly available images
          // The media worker already completed this object. Reconcile views only;
          // enqueueing another preload here would form an event-driven loop.
          coordinator.enqueue({ domains: ['posts'], peerId: event.peer_id });
          break;

        case 'media_transfer_changed':
          if (
            isMediaTransferEventForIdentity(event.profile_id, useIdentityStore.getState().state)
          ) {
            useMediaTransfersStore.getState().apply(event.state);
          }
          break;

        case 'wall_post_deleted_on_relay':
          console.log(`[Network] Wall post deleted on relay: ${event.post_id}`);
          coordinator.enqueue({ domains: ['posts'] });
          break;

        case 'call_signaling_received':
          console.log(
            '[Network] Call signaling:',
            summarizeSignalingForLog(event.message, 'inbound', 'dispatching'),
          );
          {
            const calling = useCallingStore.getState();
            const payload = event.message.payload;
            if (payload.type === 'offer') {
              notifyHarborEvent({
                kind: 'incoming_call',
                peerId: event.peer_id,
                senderName: contactName(event.peer_id),
                eventId: payload.payload.callId,
              });
            } else if (payload.type === 'group_membership' && payload.payload.action === 'invite') {
              notifyHarborEvent({
                kind: 'incoming_call',
                peerId: event.peer_id,
                senderName: contactName(event.peer_id),
                eventId: payload.payload.roomId,
                mediaMode: payload.payload.mediaMode,
              });
            } else if (
              (payload.type === 'hangup' || payload.type === 'decline') &&
              calling.runtimeSnapshot.peerId === event.peer_id &&
              ['incoming', 'ringing'].includes(calling.runtimeSnapshot.state)
            ) {
              notifyHarborEvent({
                kind: 'missed_call',
                peerId: event.peer_id,
                senderName: contactName(event.peer_id),
                eventId: payload.payload.callId,
              });
            }
            calling.handleBackendEvent(event).catch((error) => {
              console.warn('[Network] Failed to refresh call state after signaling event:', error);
            });
          }
          window.dispatchEvent(new CustomEvent('harbor:calling-signaling', { detail: event }));
          break;

        case 'call_signaling_error':
          console.warn(`[Network] Call signaling error from ${event.peer_id}: ${event.error}`);
          useCallingStore
            .getState()
            .hydrateCalls()
            .catch((error) => {
              console.warn('[Network] Failed to reconcile calls after signaling failure', error);
            });
          toast.error(`Call signaling failed: ${event.error}`);
          break;
      }
    }

    void setupListeners();

    // Cleanup on unmount
    return () => {
      listenersReady = false;
      contactFeedPoller.stop();
      unregisterProfileReset();
      storeUnsubscribers.forEach((unsubscribe) => unsubscribe());
      window.removeEventListener('online', reconcileContactFeedPoller);
      window.removeEventListener('offline', reconcileContactFeedPoller);
      coordinator.stop();
      listenerScope.dispose();
      clearProfileEventRegistration(readinessLease);
    };
  }, [profileToken, refreshPeers, refreshStats]);
}
