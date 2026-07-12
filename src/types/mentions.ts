export type MentionResolutionStatus = 'known' | 'private' | 'unknown' | 'blocked';
export type MentionIntent = 'notify' | 'repost-request';

export interface ResolvedMention {
  qualifiedName: string;
  status: MentionResolutionStatus;
  peerId?: string;
  claimDigest?: string;
}

export interface SignedMentionInput {
  qualifiedName: string;
  intent: MentionIntent;
  authorizedPeerId?: string;
  claimDigest?: string;
}

export interface PublishMentionedPostRequest {
  contentType: string;
  contentText: string;
  visibility: 'contacts' | 'public';
  mentions: SignedMentionInput[];
}

export interface MentionReceipt {
  mentionId: string;
  postId: string;
  qualifiedName: string;
  intent: MentionIntent;
  status: 'pending' | 'accepted' | 'declined' | 'blocked';
  senderPeerId: string;
  preview: string;
  createdAt: number;
}

export interface PublishMentionedPostResult {
  postId: string;
  createdAt: number;
  trackingWall?: string;
}
