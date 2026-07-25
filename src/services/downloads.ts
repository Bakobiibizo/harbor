import { invokeCommand } from './command';

/** Save generated text through Harbor's native downloads capability. */
export function saveTextToDownloads(filename: string, content: string): Promise<string> {
  return invokeCommand('save_to_downloads', { filename, content });
}
