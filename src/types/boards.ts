/** Community info from the backend */
export interface CommunityInfo {
  relayPeerId: string;
  relayAddress: string;
  communityName: string | null;
  joinedAt: number;
  lastSyncAt: number | null;
}

/** Board info from the backend */
export interface BoardInfo {
  boardId: string;
  relayPeerId: string;
  name: string;
  description: string | null;
  isDefault: boolean;
}

/** Board post info from the backend */
export interface BoardPost {
  postId: string;
  boardId: string;
  relayPeerId: string;
  authorPeerId: string;
  authorDisplayName: string | null;
  contentType: string;
  contentText: string | null;
  lamportClock: number;
  createdAt: number;
}
