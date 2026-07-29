export type MediaModality = 'image' | 'video' | 'audio';
export type PostModality = 'text' | MediaModality;
export type ModalityFilter = 'all' | 'images' | 'videos' | 'audio';

interface ModalityMedia {
  type: MediaModality;
}

/**
 * A post has one canonical modality so a mixed-media post cannot appear in
 * multiple filtered views. The first attachment is the post's primary media.
 */
export function derivePostModality(
  contentType: string | undefined,
  media?: readonly ModalityMedia[],
): PostModality {
  const primaryMedia = media?.[0]?.type;
  if (primaryMedia) return primaryMedia;
  if (contentType === 'image' || contentType === 'video' || contentType === 'audio') {
    return contentType;
  }
  return 'text';
}

export function contentTypeForPost(
  selectedContentType: 'post' | 'thought' | MediaModality,
  media?: readonly ModalityMedia[],
): 'post' | 'thought' | MediaModality {
  const modality = derivePostModality(selectedContentType, media);
  return modality === 'text' ? selectedContentType : modality;
}

export function matchesModalityFilter(
  filter: ModalityFilter,
  contentType: string | undefined,
  media?: readonly ModalityMedia[],
): boolean {
  if (filter === 'all') return true;
  const modality = derivePostModality(contentType, media);
  const selectedModality: MediaModality =
    filter === 'images' ? 'image' : filter === 'videos' ? 'video' : 'audio';
  return modality === selectedModality;
}
