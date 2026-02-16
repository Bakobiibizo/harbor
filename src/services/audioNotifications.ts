import { useSettingsStore } from '../stores/settings';
import { useMessagingStore } from '../stores/messaging';
import { useBoardsStore } from '../stores/boards';
import { createLogger } from '../utils/logger';

const log = createLogger('AudioNotifications');

/**
 * Audio notification service for Harbor.
 *
 * Plays distinct sounds for different event types:
 * - Messages: harbor-message.wav
 * - Wall/feed posts: harbor-post.wav
 * - Community board posts: harbor-community.wav
 *
 * Supports smart muting: sounds are suppressed when the user
 * is actively viewing the relevant conversation or board.
 */

// Preloaded audio elements for instant playback
let messageAudio: HTMLAudioElement | null = null;
let postAudio: HTMLAudioElement | null = null;
let communityAudio: HTMLAudioElement | null = null;

/**
 * Preload all sound files so they play instantly when needed.
 * Call this once during app initialization.
 */
export function preloadSounds(): void {
  try {
    messageAudio = new Audio('/sounds/harbor-message.wav');
    messageAudio.preload = 'auto';
    messageAudio.volume = 0.6;

    postAudio = new Audio('/sounds/harbor-post.wav');
    postAudio.preload = 'auto';
    postAudio.volume = 0.6;

    communityAudio = new Audio('/sounds/harbor-community.wav');
    communityAudio.preload = 'auto';
    communityAudio.volume = 0.6;

    log.info('Sounds preloaded');
  } catch (err) {
    log.error('Failed to preload sounds', err);
  }
}

/**
 * Internal helper to play an audio element if sound is enabled.
 */
function playSound(audio: HTMLAudioElement | null): void {
  if (!audio) {
    log.warn('Audio not preloaded, skipping playback');
    return;
  }

  // Check if sound is enabled in settings
  const { soundEnabled } = useSettingsStore.getState();
  if (!soundEnabled) {
    return;
  }

  // Clone the audio to allow overlapping playback and reset position
  try {
    audio.currentTime = 0;
    audio.play().catch((err) => {
      log.warn('Playback failed', err);
    });
  } catch (err) {
    log.warn('Error playing sound', err);
  }
}

/**
 * Play the message received sound.
 *
 * Smart muting: If the user is currently viewing the conversation
 * with the sender (activeConversation matches senderPeerId), the
 * sound is suppressed.
 *
 * @param senderPeerId - The peer ID of the message sender
 */
export function playMessageSound(senderPeerId?: string): void {
  // Smart mute: don't play if user is viewing that conversation
  if (senderPeerId) {
    const { activeConversation } = useMessagingStore.getState();
    if (activeConversation === senderPeerId) {
      log.debug('Smart mute: user is viewing this conversation');
      return;
    }
  }

  playSound(messageAudio);
}

/**
 * Play the wall/feed post notification sound.
 *
 * Smart muting: If the user is currently on the Feed page
 * (detected via URL hash), the sound is suppressed since they
 * can see posts arriving in real time.
 */
export function playPostSound(): void {
  // Smart mute: don't play if user is viewing the feed
  try {
    const hash = window.location.hash;
    if (hash.includes('/feed')) {
      log.debug('Smart mute: user is viewing the feed');
      return;
    }
  } catch {
    // Proceed with sound if we can't check location
  }

  playSound(postAudio);
}

/**
 * Play the community board post notification sound.
 *
 * Smart muting: If the user is currently viewing the specific
 * board where the post was made, the sound is suppressed.
 *
 * @param boardId - Optional board ID where the post was made
 */
export function playCommunitySound(boardId?: string): void {
  // Smart mute: don't play if user is viewing the boards page
  // and specifically the board that received the post
  try {
    const hash = window.location.hash;
    if (hash.includes('/boards')) {
      if (!boardId) {
        // If we don't know which board, mute any boards page view
        log.debug('Smart mute: user is viewing boards');
        return;
      }
      const { activeBoard } = useBoardsStore.getState();
      if (activeBoard && activeBoard.boardId === boardId) {
        log.debug('Smart mute: user is viewing this board');
        return;
      }
    }
  } catch {
    // Proceed with sound if we can't check
  }

  playSound(communityAudio);
}
