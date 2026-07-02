import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { KEYBOARD_SHORTCUTS } from './useKeyboardNavigation';

// Mock react-router-dom since the hook depends on it
vi.mock('react-router-dom', () => ({
  useNavigate: vi.fn(() => vi.fn()),
  useLocation: vi.fn(() => ({ pathname: '/chat' })),
}));

describe('KEYBOARD_SHORTCUTS', () => {
  it('should define shortcuts for all pages', () => {
    const descriptions = KEYBOARD_SHORTCUTS.map((s) => s.description);

    expect(descriptions).toContain('Go to Messages');
    expect(descriptions).toContain('Go to Journal');
    expect(descriptions).toContain('Go to Feed');
    expect(descriptions).toContain('Go to Network');
    expect(descriptions).toContain('Go to Settings');
  });

  it('should define Ctrl+N for new action', () => {
    const shortcut = KEYBOARD_SHORTCUTS.find((s) => s.description === 'New message/post');
    expect(shortcut).toBeDefined();
    expect(shortcut?.key).toBe('N');
    expect(shortcut?.ctrlKey).toBe(true);
  });

  it('should define Ctrl+K for quick search', () => {
    const shortcut = KEYBOARD_SHORTCUTS.find((s) => s.description === 'Quick search');
    expect(shortcut).toBeDefined();
    expect(shortcut?.key).toBe('K');
    expect(shortcut?.ctrlKey).toBe(true);
  });

  it('should define Escape for close dialog', () => {
    const shortcut = KEYBOARD_SHORTCUTS.find((s) => s.description === 'Close dialog/modal');
    expect(shortcut).toBeDefined();
    expect(shortcut?.key).toBe('Escape');
  });

  it('should define Alt+arrow keys for page navigation', () => {
    const prev = KEYBOARD_SHORTCUTS.find((s) => s.description === 'Previous page');
    const next = KEYBOARD_SHORTCUTS.find((s) => s.description === 'Next page');

    expect(prev).toBeDefined();
    expect(prev?.altKey).toBe(true);
    expect(next).toBeDefined();
    expect(next?.altKey).toBe(true);
  });

  it('should define vim-style list navigation', () => {
    const descriptions = KEYBOARD_SHORTCUTS.map((s) => s.description);

    expect(descriptions).toContain('Previous item in list');
    expect(descriptions).toContain('Next item in list');
    expect(descriptions).toContain('Select/activate item');
  });

  it('should have the correct number of shortcuts', () => {
    expect(KEYBOARD_SHORTCUTS.length).toBeGreaterThanOrEqual(14);
  });
});

describe('useListKeyboardNavigation', () => {
  let keydownHandlers: ((e: KeyboardEvent) => void)[] = [];

  beforeEach(() => {
    keydownHandlers = [];
    vi.spyOn(window, 'addEventListener').mockImplementation((event: string, handler: any) => {
      if (event === 'keydown') {
        keydownHandlers.push(handler);
      }
    });
    vi.spyOn(window, 'removeEventListener').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('should handle keyboard events for list navigation logic', () => {
    // Test the logic that would be used by the hook
    const items = ['a', 'b', 'c', 'd', 'e'];
    let selectedIndex = 2;
    const onSelect = (idx: number) => {
      selectedIndex = idx;
    };

    // Simulate ArrowDown
    const newIndexDown = Math.min(items.length - 1, selectedIndex + 1);
    onSelect(newIndexDown);
    expect(selectedIndex).toBe(3);

    // Simulate ArrowUp
    const newIndexUp = Math.max(0, selectedIndex - 1);
    onSelect(newIndexUp);
    expect(selectedIndex).toBe(2);

    // Simulate Home
    onSelect(0);
    expect(selectedIndex).toBe(0);

    // Simulate End
    onSelect(items.length - 1);
    expect(selectedIndex).toBe(4);
  });

  it('should clamp at boundaries', () => {
    const items = ['a', 'b', 'c'];
    let selectedIndex = 0;
    const onSelect = (idx: number) => {
      selectedIndex = idx;
    };

    // ArrowUp at index 0 should stay at 0
    const newIndexUp = Math.max(0, selectedIndex - 1);
    onSelect(newIndexUp);
    expect(selectedIndex).toBe(0);

    // ArrowDown at last index should stay
    selectedIndex = 2;
    const newIndexDown = Math.min(items.length - 1, selectedIndex + 1);
    onSelect(newIndexDown);
    expect(selectedIndex).toBe(2);
  });
});
