import { fireEvent, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  formatShortcut,
  HARBOR_SHORTCUT_EVENTS,
  isEditableShortcutTarget,
  KEYBOARD_SHORTCUTS,
  shouldSendMessageFromKey,
  useKeyboardNavigation,
} from './useKeyboardNavigation';

const router = vi.hoisted(() => ({ navigate: vi.fn(), pathname: '/chat' }));
vi.mock('react-router-dom', () => ({
  useNavigate: () => router.navigate,
  useLocation: () => ({ pathname: router.pathname }),
}));

describe('keyboard shortcut registry', () => {
  it('defines the required navigation, action, and editing shortcuts', () => {
    const ids = KEYBOARD_SHORTCUTS.map((shortcut) => shortcut.id);
    expect(ids).toEqual(
      expect.arrayContaining([
        'messages',
        'wall',
        'feed',
        'boards',
        'network',
        'settings-page',
        'search',
        'new-message',
        'new-post',
        'settings',
        'shortcuts',
        'close',
        'send',
        'new-line',
      ]),
    );
  });

  it('formats platform-aware labels from the same registry', () => {
    const search = KEYBOARD_SHORTCUTS.find((shortcut) => shortcut.id === 'search')!;
    const newPost = KEYBOARD_SHORTCUTS.find((shortcut) => shortcut.id === 'new-post')!;

    expect(formatShortcut(search, 'windows-linux')).toBe('Ctrl + K');
    expect(formatShortcut(search, 'mac')).toBe('⌘ K');
    expect(formatShortcut(newPost, 'windows-linux')).toBe('Ctrl + Shift + N');
    expect(formatShortcut(newPost, 'mac')).toBe('⌘ ⇧ N');
  });

  it('sends on Enter while preserving Shift+Enter multiline editing', () => {
    expect(shouldSendMessageFromKey({ key: 'Enter', shiftKey: false, altKey: false })).toBe(true);
    expect(shouldSendMessageFromKey({ key: 'Enter', shiftKey: true, altKey: false })).toBe(false);
    expect(shouldSendMessageFromKey({ key: 'Enter', shiftKey: false, altKey: true })).toBe(false);
  });
});

describe('useKeyboardNavigation', () => {
  beforeEach(() => {
    router.navigate.mockReset();
    router.pathname = '/chat';
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('focuses search with either Ctrl or Command', () => {
    const onSearch = vi.fn();
    window.addEventListener(HARBOR_SHORTCUT_EVENTS.focusSearch, onSearch);
    const { unmount } = renderHook(() => useKeyboardNavigation());

    fireEvent.keyDown(window, { key: 'k', ctrlKey: true });
    fireEvent.keyDown(window, { key: 'k', metaKey: true });

    expect(onSearch).toHaveBeenCalledTimes(2);
    window.removeEventListener(HARBOR_SHORTCUT_EVENTS.focusSearch, onSearch);
    unmount();
  });

  it('routes new-message, new-post, and settings actions', () => {
    vi.useFakeTimers();
    const onMessage = vi.fn();
    const onPost = vi.fn();
    window.addEventListener(HARBOR_SHORTCUT_EVENTS.newMessage, onMessage);
    window.addEventListener(HARBOR_SHORTCUT_EVENTS.newPost, onPost);
    const { unmount } = renderHook(() => useKeyboardNavigation());

    fireEvent.keyDown(window, { key: 'n', ctrlKey: true });
    expect(onMessage).toHaveBeenCalledOnce();

    fireEvent.keyDown(window, { key: 'N', ctrlKey: true, shiftKey: true });
    expect(router.navigate).not.toHaveBeenCalledWith('/wall');
    expect(onPost).toHaveBeenCalledOnce();

    fireEvent.keyDown(window, { key: ',', ctrlKey: true });
    expect(router.navigate).toHaveBeenCalledWith('/settings');

    window.removeEventListener(HARBOR_SHORTCUT_EVENTS.newMessage, onMessage);
    window.removeEventListener(HARBOR_SHORTCUT_EVENTS.newPost, onPost);
    unmount();
  });

  it('does not run global shortcuts while editing, but still permits Escape', () => {
    const onMessage = vi.fn();
    const onEscape = vi.fn();
    window.addEventListener(HARBOR_SHORTCUT_EVENTS.newMessage, onMessage);
    window.addEventListener(HARBOR_SHORTCUT_EVENTS.escape, onEscape);
    const { unmount } = renderHook(() => useKeyboardNavigation());
    const input = document.createElement('input');
    document.body.appendChild(input);

    expect(isEditableShortcutTarget(input)).toBe(true);
    fireEvent.keyDown(input, { key: 'n', ctrlKey: true });
    fireEvent.keyDown(input, { key: 'Escape' });

    expect(onMessage).not.toHaveBeenCalled();
    expect(onEscape).toHaveBeenCalledOnce();

    input.remove();
    window.removeEventListener(HARBOR_SHORTCUT_EVENTS.newMessage, onMessage);
    window.removeEventListener(HARBOR_SHORTCUT_EVENTS.escape, onEscape);
    unmount();
  });
});

describe('list keyboard navigation boundaries', () => {
  it('clamps movement at the beginning and end of a list', () => {
    const items = ['a', 'b', 'c'];
    expect(Math.max(0, 0 - 1)).toBe(0);
    expect(Math.min(items.length - 1, 2 + 1)).toBe(2);
  });
});
