import { useEffect } from 'react';
import { emit, listen } from '@tauri-apps/api/event';
import { useCallingStore, useContactsStore, useIdentityStore } from '../stores';
import { getErrorMessage } from '../utils/errors';
import { createAsyncDisposerScope } from '../utils/asyncDisposer';
import { getProfileEventsReady } from '../services/profileEventReadiness';

interface HarborControlEvent {
  id: string;
  action: string;
  payload: Record<string, unknown>;
}

export async function handleHarborControlEvent(event: HarborControlEvent) {
  const peerId = typeof event.payload.peerId === 'string' ? event.payload.peerId : '';
  const video = event.payload.video === true;
  switch (event.action) {
    case 'state.snapshot':
      return {
        identity: useIdentityStore.getState().state,
        call: useCallingStore.getState().runtimeSnapshot,
        group: useCallingStore.getState().groupRuntimeSnapshot,
        error: useCallingStore.getState().error,
        profileEventsReady: getProfileEventsReady(),
      };
    case 'identity.refresh':
      await useIdentityStore.getState().initialize();
      return useIdentityStore.getState().state;
    case 'contact.accept': {
      if (!peerId) throw new Error('contact.accept requires payload.peerId');
      const contacts = useContactsStore.getState();
      await contacts.loadRequests();
      const request = useContactsStore
        .getState()
        .requests.find(
          (item) =>
            item.peerId === peerId &&
            item.direction === 'incoming' &&
            (item.status === 'review' || item.status === 'failed'),
        );
      if (!request) throw new Error(`No pending contact request from ${peerId}`);
      return useContactsStore.getState().respondToRequest(request.requestId, 'accepted');
    }
    case 'call.start':
      if (!peerId) throw new Error('call.start requires payload.peerId');
      // Media permission and ICE setup can outlive the bounded control-response
      // window. Acknowledge startup immediately and expose progress through
      // authoritative state snapshots, just like group-call startup below.
      globalThis.setTimeout(() => {
        void useCallingStore
          .getState()
          .startOutgoingCall(peerId, { video })
          .catch((error) => console.warn('[HarborControl] Call start failed:', error));
      }, 0);
      return useCallingStore.getState().runtimeSnapshot;
    case 'call.accept':
      await useCallingStore.getState().acceptIncomingCall();
      return useCallingStore.getState().runtimeSnapshot;
    case 'call.decline':
      await useCallingStore.getState().declineIncomingCall();
      return useCallingStore.getState().runtimeSnapshot;
    case 'call.hangup':
      await useCallingStore.getState().hangupActiveCall('normal');
      return useCallingStore.getState().runtimeSnapshot;
    case 'group.start': {
      const peerIds = Array.isArray(event.payload.peerIds)
        ? event.payload.peerIds.filter((value): value is string => typeof value === 'string')
        : [];
      if (peerIds.length === 0) throw new Error('group.start requires payload.peerIds');
      // Group mesh setup can legitimately outlive the bounded control-response
      // window while multiple WebRTC legs gather ICE. Start it in the frontend
      // lifecycle and let the harness observe progress through state snapshots.
      globalThis.setTimeout(() => {
        void useCallingStore
          .getState()
          .startOutgoingGroupCall(peerIds, { video })
          .catch((error) => console.warn('[HarborControl] Group call start failed:', error));
      }, 0);
      return useCallingStore.getState().groupRuntimeSnapshot;
    }
    case 'group.accept':
      await useCallingStore.getState().acceptIncomingGroupCall();
      return useCallingStore.getState().groupRuntimeSnapshot;
    case 'group.decline':
      await useCallingStore.getState().declineIncomingGroupCall();
      return useCallingStore.getState().groupRuntimeSnapshot;
    case 'group.leave':
      await useCallingStore.getState().leaveGroupCall('normal');
      return useCallingStore.getState().groupRuntimeSnapshot;
    default:
      throw new Error(`Unknown Harbor control action: ${event.action}`);
  }
}

/**
 * Owns the authenticated Rust control bridge for the lifetime of the app.
 *
 * Rust validates the control token before emitting `harbor:control`. Keeping
 * this listener independent of a profile epoch lets the harness refresh a
 * newly created identity while profile-scoped network listeners stay isolated.
 */
export function useHarborControlEvents() {
  useEffect(() => {
    const reportListenerError = (error: unknown) => {
      console.warn('[HarborControl] Listener lifecycle failure:', error);
    };
    const listenerScope = createAsyncDisposerScope(reportListenerError);

    void listen<HarborControlEvent>('harbor:control', (event) => {
      if (listenerScope.disposed) return;
      void (async () => {
        try {
          const result = await handleHarborControlEvent(event.payload);
          await emit('harbor:control-result', { id: event.payload.id, ok: true, result });
        } catch (error) {
          await emit('harbor:control-result', {
            id: event.payload.id,
            ok: false,
            error: getErrorMessage(error),
          });
        }
      })().catch((error) => {
        console.warn('[HarborControl] Failed to publish control result:', error);
      });
    })
      .then((dispose) => listenerScope.add(dispose))
      .catch(reportListenerError);

    return () => listenerScope.dispose();
  }, []);
}
