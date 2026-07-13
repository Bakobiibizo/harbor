import { describe, expect, it } from 'vitest';
import { extractQualifiedMentions } from './mentions';

describe('extractQualifiedMentions', () => {
  it('extracts normalized qualified names without treating ordinary text as identity', () => {
    expect(
      extractQualifiedMentions('Hi @Alice@Relay.Example and @alice@relay.example. Also @local.'),
    ).toEqual(['@alice@relay.example']);
  });
  it('does not accept malformed or ambiguous names', () =>
    expect(extractQualifiedMentions('@alice @a@@relay @alice@')).toEqual([]));
});
