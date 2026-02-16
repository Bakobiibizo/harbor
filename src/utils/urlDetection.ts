/**
 * URL detection utility for extracting URLs from post content.
 */

/**
 * Regular expression to match HTTP(S) URLs in text.
 * Matches URLs starting with http:// or https:// followed by
 * valid URL characters, stopping at common sentence terminators.
 */
const URL_REGEX = /https?:\/\/[^\s<>"')\]},]+/gi;

/**
 * Extract all URLs found in a text string.
 */
export function extractUrls(text: string): string[] {
  const matches = text.match(URL_REGEX);
  if (!matches) return [];

  // Clean trailing punctuation that is likely not part of the URL
  return matches.map((url) => {
    // Remove trailing periods, commas, exclamation marks, question marks,
    // colons, semicolons that are likely sentence-ending punctuation
    return url.replace(/[.,!?;:]+$/, '');
  });
}

/**
 * Extract the first URL found in a text string.
 * Returns null if no URL is found.
 */
export function extractFirstUrl(text: string): string | null {
  const urls = extractUrls(text);
  return urls.length > 0 ? urls[0] : null;
}

/**
 * Extract the domain name from a URL for display purposes.
 * e.g. "https://www.example.com/page" -> "example.com"
 */
export function getDomainFromUrl(url: string): string {
  try {
    const parsed = new URL(url);
    // Remove "www." prefix for cleaner display
    return parsed.hostname.replace(/^www\./, '');
  } catch {
    return url;
  }
}
