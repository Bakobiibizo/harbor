import { describe, expect, it } from 'vitest';
import type { BoardPost } from '../types';
import { boardAuthorLabel } from './Boards';

function post(overrides: Partial<BoardPost> = {}): BoardPost {
  return {
    postId: 'post-1',
    boardId: 'board-1',
    relayPeerId: 'relay-1',
    authorPeerId: 'peer-1',
    authorDisplayName: 'Relay supplied alias',
    authorVerifiedQualifiedName: null,
    contentType: 'text',
    contentText: 'Hello',
    lamportClock: 1,
    createdAt: 1,
    ...overrides,
  };
}

describe('board author identity presentation', () => {
  it('shows a backend-verified relay claim', () => {
    expect(boardAuthorLabel(post({ authorVerifiedQualifiedName: '@alice@relay.test' }))).toBe(
      '@alice@relay.test',
    );
  });

  it('marks relay-supplied and forged aliases unverified', () => {
    expect(boardAuthorLabel(post())).toBe('Relay supplied alias@unverified');
    expect(boardAuthorLabel(post({ authorDisplayName: '@alice@relay.test' }))).toBe(
      '@alice@relay.test@unverified',
    );
  });
});
