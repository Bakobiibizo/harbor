import type {
  MentionReceipt,
  PublishMentionedPostRequest,
  PublishMentionedPostResult,
  ResolvedMention,
} from '../types';
import { invokeCommand } from './command';

export const mentionsService = {
  resolve(qualifiedName: string): Promise<ResolvedMention> {
    return invokeCommand('resolve_private_mention', { qualifiedName });
  },
  publish(request: PublishMentionedPostRequest): Promise<PublishMentionedPostResult> {
    return invokeCommand('create_post_with_mentions', { request });
  },
  listPending(): Promise<MentionReceipt[]> {
    return invokeCommand('list_pending_mentions');
  },
  review(
    mentionId: string,
    decision: 'accept-notification' | 'accept-repost' | 'decline' | 'block',
  ): Promise<void> {
    return invokeCommand('review_private_mention', { mentionId, decision });
  },
};
