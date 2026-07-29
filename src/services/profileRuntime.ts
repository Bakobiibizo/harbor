import { useBoardsStore } from '../stores/boards';
import { useCallingStore } from '../stores/calling';
import { useContactsStore } from '../stores/contacts';
import { useContactWallStore } from '../stores/contactWall';
import { useIdentityStore } from '../stores/identity';
import { useMediaTransfersStore } from '../stores/mediaTransfers';
import { useNetworkStore } from '../stores/network';
import { useWallStore } from '../stores/wall';
import { clearLinkPreviewCache } from '../components/common/LinkPreviewCard';
import { clearProviderSessionConsent } from '../utils/providerEmbeds';
import { resetRegisteredProfileResources } from './profileRuntimeLifecycle';

/**
 * Synchronously remove all process-memory state owned by the active profile.
 * Backend network/key teardown remains authoritative and must happen before this
 * coordinator is called. The only identity data retained is its public locked
 * summary so the unlock screen can identify the local account safely.
 */
export function resetProfileRuntime(): void {
  resetRegisteredProfileResources();
  useCallingStore.getState().reset();
  // Lock the identity view before resetting subscribed peer stores so their
  // observers cannot briefly restart profile-bound work during teardown.
  useIdentityStore.getState().resetRuntimeSession();
  useNetworkStore.getState().reset();
  useContactsStore.getState().reset();
  useBoardsStore.getState().reset();
  useWallStore.getState().reset();
  useContactWallStore.getState().reset();
  useMediaTransfersStore.getState().reset();
  clearProviderSessionConsent();
  clearLinkPreviewCache();
}
