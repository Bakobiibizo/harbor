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
}

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
}

/** Result of creating a post */
export interface CreatePostResult {
  postId: string;
  createdAt: number;
}
