import { invokeCommand, type LinkPreviewData } from './command';

export type { LinkPreviewData } from './command';

/** Fetch sanitized preview metadata through Harbor's native network boundary. */
export function fetchLinkPreview(url: string): Promise<LinkPreviewData> {
  return invokeCommand('fetch_link_preview', { url });
}
