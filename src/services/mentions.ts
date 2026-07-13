import { invoke } from '@tauri-apps/api/core';
import type {
  MentionReceipt,
  PublishMentionedPostRequest,
  PublishMentionedPostResult,
  ResolvedMention,
} from '../types';

export const mentionsService = {
  resolve(qualifiedName: string): Promise<ResolvedMention> {
    return invoke('resolve_private_mention', { qualifiedName });
  },
  publish(request: PublishMentionedPostRequest): Promise<PublishMentionedPostResult> {
    return invoke('create_post_with_mentions', { request });
  },
  listPending(): Promise<MentionReceipt[]> {
    return invoke('list_pending_mentions');
  },
  review(
    mentionId: string,
    decision: 'accept-notification' | 'accept-repost' | 'decline' | 'block',
  ): Promise<void> {
    return invoke('review_private_mention', { mentionId, decision });
  },
};
