import { hydrateSettingsProfile, resetSettingsProfileMemory } from '../stores/settings';
import { hydrateMessagingProfile, resetMessagingProfileMemory } from '../stores/messaging';
import {
  hydrateNotificationsProfile,
  resetNotificationsProfileMemory,
} from '../stores/notifications';
import { hydrateFeedProfile, resetFeedProfileMemory } from '../stores/feed';

/** Hydrate every durable profile namespace after authoritative activation. */
export async function hydrateProfilePersistence(): Promise<void> {
  await hydrateSettingsProfile();
  hydrateMessagingProfile();
  hydrateNotificationsProfile();
  hydrateFeedProfile();
}

/** Clear profile-owned values without writing after session suspension. */
export function resetProfilePersistenceMemory(): void {
  resetSettingsProfileMemory();
  resetMessagingProfileMemory();
  resetNotificationsProfileMemory();
  resetFeedProfileMemory();
}
