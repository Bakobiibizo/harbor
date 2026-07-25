/** A wall/blog post */
export interface Post {
  postId: string;
  authorPeerId: string;
  contentType: string;
  contentText: string | null;
  visibility: PostVisibility;
  lamportClock: number;
  createdAt: number;
  updatedAt: number;
  deletedAt: number | null;
  isLocal: boolean;
  relayStatus: PostRelayStatus;
}

export type PostRelayStatus = 'local_pending' | 'relay_acknowledged' | 'conflict' | 'failed';

/** Post visibility setting */
export type PostVisibility = 'contacts' | 'public';

/** Post media attachment */
export interface PostMedia {
  id: number;
  postId: string;
  mediaHash: string;
  mediaType: string;
  mimeType: string;
  fileName: string;
  fileSize: number;
  width: number | null;
  height: number | null;
  durationSeconds: number | null;
  sortOrder: number;
  signature: number[];
}

/** Media metadata used when creating a signed media post */
export interface CreatePostMediaInput {
  mediaHash: string;
  mediaType: 'image' | 'video' | 'audio';
  mimeType: string;
  fileName: string;
  fileSize: number;
  width?: number;
  height?: number;
  durationSeconds?: number;
  sortOrder: number;
}

/** Result of creating a post */
export interface CreatePostResult {
  postId: string;
  createdAt: number;
  relayStatus: PostRelayStatus;
}

export interface PostMutationResult {
  postId: string;
  relayStatus: PostRelayStatus;
}
