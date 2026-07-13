import { describe, expect, it } from 'vitest';
import type { IdentityInfo, IdentityState } from '../types';
import { isMediaTransferEventForIdentity } from './mediaTransferEvents';

function unlocked(peerId: string): IdentityState {
  return {
    status: 'unlocked',
    identity: { peerId } as IdentityInfo,
  };
}

describe('media transfer event profile isolation', () => {
  it('rejects a delayed profile A event after profile B becomes active', () => {
    expect(isMediaTransferEventForIdentity('profile-a', unlocked('profile-b'))).toBe(false);
    expect(isMediaTransferEventForIdentity('profile-b', unlocked('profile-b'))).toBe(true);
  });

  it('rejects transfer events while identity keys are locked', () => {
    const locked: IdentityState = {
      status: 'locked',
      identity: { peerId: 'profile-a' } as IdentityInfo,
    };
    expect(isMediaTransferEventForIdentity('profile-a', locked)).toBe(false);
  });
});
