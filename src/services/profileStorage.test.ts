import { activateProfile, suspendProfile } from './profileSession';
import { migrateLegacyProfileValue, profileStorageKey } from './profileStorage';

describe('profileStorage', () => {
  beforeEach(() => {
    suspendProfile();
    localStorage.clear();
  });

  it('creates isolated keys for A, B, and A again', () => {
    activateProfile('peer/a');
    const aKey = profileStorageKey('settings', 2);
    localStorage.setItem(aKey, 'alice');

    activateProfile('peer-b');
    const bKey = profileStorageKey('settings', 2);
    localStorage.setItem(bKey, 'bob');

    activateProfile('peer/a');
    expect(profileStorageKey('settings', 2)).toBe(aKey);
    expect(localStorage.getItem(aKey)).toBe('alice');
    expect(localStorage.getItem(bKey)).toBe('bob');
    expect(aKey).toBe('harbor:profile:peer%2Fa:settings:v2');
  });

  it('does not access or migrate storage before activation', () => {
    localStorage.setItem('legacy-settings', 'legacy');
    expect(() => profileStorageKey('settings', 1)).toThrow('No Harbor profile is active');
    expect(() => migrateLegacyProfileValue('legacy-settings', 'settings', 1)).toThrow(
      'No Harbor profile is active',
    );
    expect(localStorage.getItem('legacy-settings')).toBe('legacy');
  });

  it('migrates a legacy value once into only the active profile', () => {
    localStorage.setItem('legacy-settings', '{"theme":"dark"}');
    activateProfile('peer-a');

    expect(migrateLegacyProfileValue('legacy-settings', 'settings', 1)).toBe('{"theme":"dark"}');
    const aKey = profileStorageKey('settings', 1);
    expect(localStorage.getItem(aKey)).toBe('{"theme":"dark"}');
    expect(localStorage.getItem('legacy-settings')).toBeNull();

    activateProfile('peer-b');
    expect(migrateLegacyProfileValue('legacy-settings', 'settings', 1)).toBeNull();
    expect(localStorage.getItem(profileStorageKey('settings', 1))).toBeNull();
  });
});
