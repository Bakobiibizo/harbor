import {
  activateProfile,
  captureProfile,
  isCurrentProfile,
  onProfileSuspend,
  requireProfileId,
  suspendProfile,
} from './profileSession';

describe('profileSession', () => {
  beforeEach(() => suspendProfile());

  it('invalidates delayed work when switching A to B and back to A', () => {
    const firstA = activateProfile('peer-a');
    expect(Object.isFrozen(firstA)).toBe(true);
    expect(requireProfileId()).toBe('peer-a');

    const b = activateProfile('peer-b');
    expect(isCurrentProfile(firstA)).toBe(false);
    expect(isCurrentProfile(b)).toBe(true);

    const secondA = activateProfile('peer-a');
    expect(secondA.epoch).toBeGreaterThan(firstA.epoch);
    expect(isCurrentProfile(b)).toBe(false);
    expect(captureProfile()).toBe(secondA);
  });

  it('suspends each epoch exactly once', () => {
    const callback = vi.fn();
    const unsubscribe = onProfileSuspend(callback);
    const token = activateProfile('peer-a');

    suspendProfile();
    suspendProfile();

    expect(callback).toHaveBeenCalledOnce();
    expect(callback).toHaveBeenCalledWith(token);
    expect(captureProfile()).toBeNull();
    expect(() => requireProfileId()).toThrow('No Harbor profile is active');
    unsubscribe();
  });
});
