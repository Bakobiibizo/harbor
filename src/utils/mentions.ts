const QUALIFIED_MENTION =
  /(^|\s)@([a-z0-9_-]{1,32})@([a-z0-9](?:[a-z0-9.-]*[a-z0-9])?)(?=$|[\s.,!?;:])/gi;

export function extractQualifiedMentions(text: string): string[] {
  const found = new Set<string>();
  for (const match of text.matchAll(QUALIFIED_MENTION))
    found.add(`@${match[2].toLowerCase()}@${match[3].toLowerCase()}`);
  return [...found];
}
