import { useEffect, useCallback } from 'react';
import { useNavigate, useLocation } from 'react-router-dom';

export type ShortcutPlatform = 'mac' | 'windows-linux';
export type ShortcutCategory = 'Navigation' | 'Actions' | 'Editing';

export interface KeyboardShortcut {
  id: string;
  key: string;
  modifier?: 'mod' | 'alt';
  altKey?: boolean;
  shiftKey?: boolean;
  description: string;
  category: ShortcutCategory;
}

export const HARBOR_SHORTCUT_EVENTS = {
  focusSearch: 'harbor:focus-search',
  newMessage: 'harbor:new-message',
  newPost: 'harbor:new-post',
  escape: 'harbor:escape',
  showShortcuts: 'harbor:show-shortcuts',
} as const;

const PAGE_ROUTES = ['/chat', '/wall', '/feed', '/boards', '/network', '/settings'] as const;

export function getShortcutPlatform(platform = navigator.platform): ShortcutPlatform {
  return /Mac|iPhone|iPad|iPod/i.test(platform) ? 'mac' : 'windows-linux';
}

export function formatShortcut(
  shortcut: KeyboardShortcut,
  platform: ShortcutPlatform = getShortcutPlatform(),
): string {
  const parts: string[] = [];
  if (shortcut.modifier === 'mod') parts.push(platform === 'mac' ? '⌘' : 'Ctrl');
  if (shortcut.modifier === 'alt' || shortcut.altKey) parts.push(platform === 'mac' ? '⌥' : 'Alt');
  if (shortcut.shiftKey) parts.push(platform === 'mac' ? '⇧' : 'Shift');
  parts.push(shortcut.key);
  return parts.join(platform === 'mac' ? ' ' : ' + ');
}

export function isEditableShortcutTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return (
    target.tagName === 'INPUT' ||
    target.tagName === 'TEXTAREA' ||
    target.tagName === 'SELECT' ||
    target.isContentEditable ||
    Boolean(target.closest('[contenteditable="true"], [role="textbox"]'))
  );
}

export function shouldSendMessageFromKey(event: {
  key: string;
  shiftKey: boolean;
  altKey: boolean;
}): boolean {
  return event.key === 'Enter' && !event.shiftKey && !event.altKey;
}

export function useKeyboardNavigation() {
  const navigate = useNavigate();
  const location = useLocation();

  // Navigate to page by index (1-5)
  const navigateToPage = useCallback(
    (index: number) => {
      if (index >= 0 && index < PAGE_ROUTES.length) {
        navigate(PAGE_ROUTES[index]);
      }
    },
    [navigate],
  );

  // Get current page index
  const getCurrentPageIndex = useCallback(() => {
    return PAGE_ROUTES.findIndex((route) => location.pathname.startsWith(route));
  }, [location.pathname]);

  // Navigate to next/previous page
  const navigateRelative = useCallback(
    (direction: 'next' | 'prev') => {
      const currentIndex = getCurrentPageIndex();
      if (currentIndex === -1) return;

      const newIndex =
        direction === 'next'
          ? Math.min(currentIndex + 1, PAGE_ROUTES.length - 1)
          : Math.max(currentIndex - 1, 0);

      if (newIndex !== currentIndex) {
        navigate(PAGE_ROUTES[newIndex]);
      }
    },
    [getCurrentPageIndex, navigate],
  );

  useEffect(() => {
    const dispatchAfterNavigation = (route: string, eventName: string) => {
      if (!location.pathname.startsWith(route)) {
        navigate(route);
        window.setTimeout(() => window.dispatchEvent(new CustomEvent(eventName)), 0);
        return;
      }
      window.dispatchEvent(new CustomEvent(eventName));
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      const isEditing = isEditableShortcutTarget(event.target);

      // Escape is intentional while typing: it dismisses the top-level overlay without editing text.
      if (event.key === 'Escape') {
        window.dispatchEvent(new CustomEvent(HARBOR_SHORTCUT_EVENTS.escape));
        return;
      }

      // All remaining global shortcuts yield to text editing and assistive technology inputs.
      if (isEditing) return;

      const modKey = event.ctrlKey || event.metaKey;

      // Cmd/Ctrl+1-6: Navigate to primary pages.
      if (modKey && !event.altKey && !event.shiftKey) {
        const keyNum = parseInt(event.key);
        if (keyNum >= 1 && keyNum <= PAGE_ROUTES.length) {
          event.preventDefault();
          navigateToPage(keyNum - 1);
          return;
        }

        // Cmd/Ctrl+N: begin a new direct message.
        if (event.key === 'n' || event.key === 'N') {
          event.preventDefault();
          dispatchAfterNavigation('/chat', HARBOR_SHORTCUT_EVENTS.newMessage);
          return;
        }

        // Cmd/Ctrl+K: focus Harbor search rather than browser chrome.
        if (event.key === 'k' || event.key === 'K') {
          event.preventDefault();
          dispatchAfterNavigation('/chat', HARBOR_SHORTCUT_EVENTS.focusSearch);
          return;
        }

        // Cmd/Ctrl+,: standard application settings shortcut.
        if (event.key === ',') {
          event.preventDefault();
          navigate('/settings');
          return;
        }

        if (event.key === '/') {
          event.preventDefault();
          window.dispatchEvent(new CustomEvent(HARBOR_SHORTCUT_EVENTS.showShortcuts));
          return;
        }
      }

      // Cmd/Ctrl+Shift+N: open the post composer.
      if (modKey && event.shiftKey && !event.altKey && (event.key === 'n' || event.key === 'N')) {
        event.preventDefault();
        dispatchAfterNavigation('/wall', HARBOR_SHORTCUT_EVENTS.newPost);
        return;
      }

      // Alt+Left/Right: Navigate pages (when not in input)
      if (event.altKey && !modKey) {
        if (event.key === 'ArrowLeft') {
          event.preventDefault();
          navigateRelative('prev');
          return;
        }
        if (event.key === 'ArrowRight') {
          event.preventDefault();
          navigateRelative('next');
          return;
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [location.pathname, navigate, navigateToPage, navigateRelative]);

  return {
    navigateToPage,
    navigateRelative,
    getCurrentPageIndex,
  };
}

// Hook for list keyboard navigation
export function useListKeyboardNavigation<T>(
  items: T[],
  selectedIndex: number,
  onSelect: (index: number) => void,
  onActivate?: (item: T) => void,
) {
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      // Don't handle if typing in input
      const target = event.target as HTMLElement;
      if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable) {
        return;
      }

      switch (event.key) {
        case 'ArrowUp':
        case 'k': // Vim-style
          event.preventDefault();
          onSelect(Math.max(0, selectedIndex - 1));
          break;
        case 'ArrowDown':
        case 'j': // Vim-style
          event.preventDefault();
          onSelect(Math.min(items.length - 1, selectedIndex + 1));
          break;
        case 'Enter':
          if (selectedIndex >= 0 && selectedIndex < items.length && onActivate) {
            event.preventDefault();
            onActivate(items[selectedIndex]);
          }
          break;
        case 'Home':
          event.preventDefault();
          onSelect(0);
          break;
        case 'End':
          event.preventDefault();
          onSelect(items.length - 1);
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [items, selectedIndex, onSelect, onActivate]);
}

// Keyboard shortcuts info for help display
export const KEYBOARD_SHORTCUTS: KeyboardShortcut[] = [
  {
    id: 'messages',
    key: '1',
    modifier: 'mod',
    description: 'Go to Messages',
    category: 'Navigation',
  },
  { id: 'wall', key: '2', modifier: 'mod', description: 'Go to My Wall', category: 'Navigation' },
  { id: 'feed', key: '3', modifier: 'mod', description: 'Go to Feed', category: 'Navigation' },
  { id: 'boards', key: '4', modifier: 'mod', description: 'Go to Boards', category: 'Navigation' },
  {
    id: 'network',
    key: '5',
    modifier: 'mod',
    description: 'Go to Network',
    category: 'Navigation',
  },
  {
    id: 'settings-page',
    key: '6',
    modifier: 'mod',
    description: 'Go to Settings',
    category: 'Navigation',
  },
  {
    id: 'previous-page',
    key: '←',
    modifier: 'alt',
    description: 'Previous page',
    category: 'Navigation',
  },
  { id: 'next-page', key: '→', modifier: 'alt', description: 'Next page', category: 'Navigation' },
  { id: 'search', key: 'K', modifier: 'mod', description: 'Focus search', category: 'Actions' },
  { id: 'new-message', key: 'N', modifier: 'mod', description: 'New message', category: 'Actions' },
  {
    id: 'new-post',
    key: 'N',
    modifier: 'mod',
    shiftKey: true,
    description: 'New post',
    category: 'Actions',
  },
  { id: 'settings', key: ',', modifier: 'mod', description: 'Open Settings', category: 'Actions' },
  {
    id: 'shortcuts',
    key: '/',
    modifier: 'mod',
    description: 'Show shortcuts',
    category: 'Actions',
  },
  { id: 'close', key: 'Esc', description: 'Close or cancel', category: 'Actions' },
  { id: 'send', key: 'Enter', description: 'Send message', category: 'Editing' },
  {
    id: 'new-line',
    key: 'Enter',
    shiftKey: true,
    description: 'New line in message',
    category: 'Editing',
  },
  {
    id: 'send-alt',
    key: 'Enter',
    modifier: 'mod',
    description: 'Send message',
    category: 'Editing',
  },
];
