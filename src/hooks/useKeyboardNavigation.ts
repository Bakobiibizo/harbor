import { useEffect, useCallback } from 'react';
import { useNavigate, useLocation } from 'react-router-dom';

interface KeyboardShortcut {
  key: string;
  ctrlKey?: boolean;
  altKey?: boolean;
  shiftKey?: boolean;
  action: () => void;
  description: string;
}

const PAGE_ROUTES = ['/chat', '/wall', '/feed', '/network', '/settings'] as const;

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
    const handleKeyDown = (event: KeyboardEvent) => {
      // Don't trigger shortcuts when typing in input fields
      const target = event.target as HTMLElement;
      const isInputField =
        target.tagName === 'INPUT' ||
        target.tagName === 'TEXTAREA' ||
        target.isContentEditable;

      // Ctrl+1-5: Navigate to pages
      if (event.ctrlKey && !event.altKey && !event.shiftKey) {
        const keyNum = parseInt(event.key);
        if (keyNum >= 1 && keyNum <= 5) {
          event.preventDefault();
          navigateToPage(keyNum - 1);
          return;
        }

        // Ctrl+N: New action (depends on current page)
        if (event.key === 'n' || event.key === 'N') {
          event.preventDefault();
          // Dispatch custom event for page-specific handling
          window.dispatchEvent(new CustomEvent('harbor:new-action'));
          return;
        }

        // Ctrl+K: Quick search (future feature)
        if (event.key === 'k' || event.key === 'K') {
          event.preventDefault();
          window.dispatchEvent(new CustomEvent('harbor:quick-search'));
          return;
        }
      }

      // Escape: Close modals/dialogs
      if (event.key === 'Escape') {
        window.dispatchEvent(new CustomEvent('harbor:escape'));
        return;
      }

      // Alt+Left/Right: Navigate pages (when not in input)
      if (event.altKey && !isInputField) {
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

      // Keyboard hints overlay (Ctrl+/)
      if (event.ctrlKey && event.key === '/') {
        event.preventDefault();
        window.dispatchEvent(new CustomEvent('harbor:show-shortcuts'));
        return;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [navigateToPage, navigateRelative]);

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
      if (
        target.tagName === 'INPUT' ||
        target.tagName === 'TEXTAREA' ||
        target.isContentEditable
      ) {
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
  { key: '1', ctrlKey: true, action: () => {}, description: 'Go to Messages' },
  { key: '2', ctrlKey: true, action: () => {}, description: 'Go to Journal' },
  { key: '3', ctrlKey: true, action: () => {}, description: 'Go to Feed' },
  { key: '4', ctrlKey: true, action: () => {}, description: 'Go to Network' },
  { key: '5', ctrlKey: true, action: () => {}, description: 'Go to Settings' },
  { key: 'N', ctrlKey: true, action: () => {}, description: 'New message/post' },
  { key: 'K', ctrlKey: true, action: () => {}, description: 'Quick search' },
  { key: '/', ctrlKey: true, action: () => {}, description: 'Show shortcuts' },
  { key: 'Escape', action: () => {}, description: 'Close dialog/modal' },
  { key: '←', altKey: true, action: () => {}, description: 'Previous page' },
  { key: '→', altKey: true, action: () => {}, description: 'Next page' },
  { key: '↑/k', action: () => {}, description: 'Previous item in list' },
  { key: '↓/j', action: () => {}, description: 'Next item in list' },
  { key: 'Enter', action: () => {}, description: 'Select/activate item' },
];
