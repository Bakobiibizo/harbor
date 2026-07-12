/** A feed item (post with author context) */
export interface FeedItem {
  postId: string;
  authorPeerId: string;
  authorDisplayName: string | null;
  authorVerifiedQualifiedName?: string | null;
  contentType: string;
  contentText: string | null;
  visibility: string;
  lamportClock: number;
  createdAt: number;
  updatedAt: number;
  isLocal: boolean;
  /** Durable signed social reaction count loaded from the local reactions table. */
  likes?: number;
  /** Whether the unlocked local identity has liked this post. */
  likedByUser?: boolean;
}
