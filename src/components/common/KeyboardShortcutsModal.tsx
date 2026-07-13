import { useEffect, useRef, useState } from 'react';
import { XIcon } from '../icons';
import {
  formatShortcut,
  getShortcutPlatform,
  HARBOR_SHORTCUT_EVENTS,
  KEYBOARD_SHORTCUTS,
  type ShortcutCategory,
} from '../../hooks/useKeyboardNavigation';

export function KeyboardShortcutsModal() {
  const [isOpen, setIsOpen] = useState(false);
  const closeButtonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    const handleShowShortcuts = () => setIsOpen(true);
    const handleEscape = () => setIsOpen(false);

    window.addEventListener(HARBOR_SHORTCUT_EVENTS.showShortcuts, handleShowShortcuts);
    window.addEventListener(HARBOR_SHORTCUT_EVENTS.escape, handleEscape);

    return () => {
      window.removeEventListener(HARBOR_SHORTCUT_EVENTS.showShortcuts, handleShowShortcuts);
      window.removeEventListener(HARBOR_SHORTCUT_EVENTS.escape, handleEscape);
    };
  }, []);

  useEffect(() => {
    if (isOpen) closeButtonRef.current?.focus();
  }, [isOpen]);

  if (!isOpen) return null;

  const platform = getShortcutPlatform();
  const categories: ShortcutCategory[] = ['Navigation', 'Actions', 'Editing'];

  return (
    <div
      className="fixed inset-0 flex items-center justify-center z-50 p-4"
      style={{ background: 'rgba(0, 0, 0, 0.6)' }}
      onClick={() => setIsOpen(false)}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="keyboard-shortcuts-title"
        className="w-full max-w-lg rounded-lg overflow-hidden"
        style={{
          background: 'hsl(var(--harbor-bg-elevated))',
          border: '1px solid hsl(var(--harbor-border-subtle))',
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div
          className="px-6 py-4 flex items-center justify-between border-b"
          style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
        >
          <h3
            id="keyboard-shortcuts-title"
            className="text-lg font-semibold"
            style={{ color: 'hsl(var(--harbor-text-primary))' }}
          >
            Keyboard Shortcuts
          </h3>
          <button
            ref={closeButtonRef}
            type="button"
            aria-label="Close keyboard shortcuts"
            onClick={() => setIsOpen(false)}
            className="p-1 rounded-lg transition-colors duration-200"
            style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
          >
            <XIcon className="w-5 h-5" />
          </button>
        </div>

        {/* Content */}
        <div className="p-6 space-y-6 max-h-[70vh] overflow-y-auto">
          {categories.map((category) => (
            <section key={category} aria-labelledby={`shortcuts-${category.toLowerCase()}`}>
              <h4
                id={`shortcuts-${category.toLowerCase()}`}
                className="text-sm font-medium mb-3"
                style={{ color: 'hsl(var(--harbor-text-secondary))' }}
              >
                {category}
              </h4>
              <div className="space-y-2">
                {KEYBOARD_SHORTCUTS.filter((shortcut) => shortcut.category === category).map(
                  (shortcut) => (
                    <div key={shortcut.id} className="flex items-center justify-between gap-5">
                      <span
                        className="text-sm"
                        style={{ color: 'hsl(var(--harbor-text-primary))' }}
                      >
                        {shortcut.description}
                      </span>
                      <kbd
                        className="px-2 py-1 rounded text-xs font-mono whitespace-nowrap"
                        style={{
                          background: 'hsl(var(--harbor-surface-1))',
                          border: '1px solid hsl(var(--harbor-border-subtle))',
                          color: 'hsl(var(--harbor-text-secondary))',
                        }}
                      >
                        {formatShortcut(shortcut, platform)}
                      </kbd>
                    </div>
                  ),
                )}
              </div>
            </section>
          ))}
        </div>

        {/* Footer */}
        <div
          className="px-6 py-3 border-t text-center"
          style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
        >
          <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
            Press{' '}
            <kbd className="px-1 rounded" style={{ background: 'hsl(var(--harbor-surface-1))' }}>
              {formatShortcut(
                KEYBOARD_SHORTCUTS.find((shortcut) => shortcut.id === 'close')!,
                platform,
              )}
            </kbd>{' '}
            to close
          </p>
        </div>
      </div>
    </div>
  );
}
