import { describe, expect, it } from 'vitest';
import {
  MAX_CONTACT_INVITE_INPUT_LENGTH,
  normalizeContactInvite,
  parseContactInvite,
} from './contactInvite';

const bundle = {
  multiaddr: '/dns4/relay.social-harbor.com/tcp/443/wss/p2p/12D3KooWExample',
  display_name: 'José',
  public_key: 'QUJDRA==',
  x25519_public: 'RUZHSA==',
  bio: 'Hello',
};

function payload(value: unknown = bundle): string {
  const bytes = new TextEncoder().encode(JSON.stringify(value));
  let binary = '';
  bytes.forEach((byte) => (binary += String.fromCharCode(byte)));
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

describe('contact invite normalization', () => {
  it('normalizes native, native handoff, and official HTTPS forms', () => {
    const encoded = payload();
    const canonical = `harbor://${encoded}`;
    expect(normalizeContactInvite(canonical)).toBe(canonical);
    expect(normalizeContactInvite(`harbor://add-friend/${encoded}`)).toBe(canonical);
    expect(normalizeContactInvite(`https://social-harbor.com/add-friend/${encoded}`)).toBe(
      canonical,
    );
    expect(normalizeContactInvite(`https://www.social-harbor.com/add-friend/${encoded}/`)).toBe(
      canonical,
    );
  });

  it('decodes UTF-8 and the backend snake_case bundle fields for previews', () => {
    expect(parseContactInvite(`harbor://${payload()}`)).toMatchObject({
      displayName: 'José',
      peerId: '12D3KooWExample',
      bio: 'Hello',
    });
  });

  it.each([
    'http://social-harbor.com/add-friend/abc',
    'https://evil.example/add-friend/abc',
    'https://social-harbor.com.evil.example/add-friend/abc',
    'https://social-harbor.com/other/abc',
    'https://social-harbor.com/add-friend/abc?next=evil',
    'harbor://not/base64!',
  ])('rejects an untrusted or malformed form: %s', (input) => {
    expect(() => normalizeContactInvite(input)).toThrow(/Invalid contact invite/);
  });

  it('rejects malformed bundles and oversized input before invoking the backend', () => {
    expect(() =>
      normalizeContactInvite(`harbor://${payload({ display_name: 'No keys' })}`),
    ).toThrow(/network address is malformed/);
    expect(() => normalizeContactInvite('x'.repeat(MAX_CONTACT_INVITE_INPUT_LENGTH + 1))).toThrow(
      /too large/,
    );
  });
});
