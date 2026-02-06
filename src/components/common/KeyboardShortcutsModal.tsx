import { useEffect, useState } from 'react';
import { XIcon } from '../icons';
import { KEYBOARD_SHORTCUTS } from '../../hooks/useKeyboardNavigation';

export function KeyboardShortcutsModal() {
  const [isOpen, setIsOpen] = useState(false);

  useEffect(() => {
    const handleShowShortcuts = () => setIsOpen(true);
    const handleEscape = () => setIsOpen(false);

    window.addEventListener('harbor:show-shortcuts', handleShowShortcuts);
    window.addEventListener('harbor:escape', handleEscape);

    return () => {
      window.removeEventListener('harbor:show-shortcuts', handleShowShortcuts);
      window.removeEventListener('harbor:escape', handleEscape);
    };
  }, []);

  if (!isOpen) return null;

  const formatKey = (shortcut: (typeof KEYBOARD_SHORTCUTS)[0]) => {
    const parts: string[] = [];
    if (shortcut.ctrlKey) parts.push('Ctrl');
    if (shortcut.altKey) parts.push('Alt');
    if (shortcut.shiftKey) parts.push('Shift');
    parts.push(shortcut.key);
    return parts.join(' + ');
  };

  // Group shortcuts by category
  const navigationShortcuts = KEYBOARD_SHORTCUTS.filter(
    (s) => s.description.includes('Go to') || s.description.includes('page'),
  );
  const actionShortcuts = KEYBOARD_SHORTCUTS.filter(
    (s) =>
      s.description.includes('New') ||
      s.description.includes('search') ||
      s.description.includes('Close') ||
      s.description.includes('Show'),
  );
  const listShortcuts = KEYBOARD_SHORTCUTS.filter(
    (s) =>
      s.description.includes('item') ||
      s.description.includes('Select') ||
      s.description.includes('Previous item') ||
      s.description.includes('Next item'),
  );

  return (
    <div
      className="fixed inset-0 flex items-center justify-center z-50 p-4"
      style={{ background: 'rgba(0, 0, 0, 0.6)' }}
      onClick={() => setIsOpen(false)}
    >
      <div
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
            className="text-lg font-semibold"
            style={{ color: 'hsl(var(--harbor-text-primary))' }}
          >
            Keyboard Shortcuts
          </h3>
          <button
            onClick={() => setIsOpen(false)}
            className="p-1 rounded-lg transition-colors duration-200"
            style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
          >
            <XIcon className="w-5 h-5" />
          </button>
        </div>

        {/* Content */}
        <div className="p-6 space-y-6 max-h-[70vh] overflow-y-auto">
          {/* Navigation */}
          <div>
            <h4
              className="text-sm font-medium mb-3"
              style={{ color: 'hsl(var(--harbor-text-secondary))' }}
            >
              Navigation
            </h4>
            <div className="space-y-2">
              {navigationShortcuts.map((shortcut, index) => (
                <div key={index} className="flex items-center justify-between">
                  <span className="text-sm" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
                    {shortcut.description}
                  </span>
                  <kbd
                    className="px-2 py-1 rounded text-xs font-mono"
                    style={{
                      background: 'hsl(var(--harbor-surface-1))',
                      border: '1px solid hsl(var(--harbor-border-subtle))',
                      color: 'hsl(var(--harbor-text-secondary))',
                    }}
                  >
                    {formatKey(shortcut)}
                  </kbd>
                </div>
              ))}
            </div>
          </div>

          {/* Actions */}
          <div>
            <h4
              className="text-sm font-medium mb-3"
              style={{ color: 'hsl(var(--harbor-text-secondary))' }}
            >
              Actions
            </h4>
            <div className="space-y-2">
              {actionShortcuts.map((shortcut, index) => (
                <div key={index} className="flex items-center justify-between">
                  <span className="text-sm" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
                    {shortcut.description}
                  </span>
                  <kbd
                    className="px-2 py-1 rounded text-xs font-mono"
                    style={{
                      background: 'hsl(var(--harbor-surface-1))',
                      border: '1px solid hsl(var(--harbor-border-subtle))',
                      color: 'hsl(var(--harbor-text-secondary))',
                    }}
                  >
                    {formatKey(shortcut)}
                  </kbd>
                </div>
              ))}
            </div>
          </div>

          {/* List Navigation */}
          <div>
            <h4
              className="text-sm font-medium mb-3"
              style={{ color: 'hsl(var(--harbor-text-secondary))' }}
            >
              List Navigation
            </h4>
            <div className="space-y-2">
              {listShortcuts.map((shortcut, index) => (
                <div key={index} className="flex items-center justify-between">
                  <span className="text-sm" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
                    {shortcut.description}
                  </span>
                  <kbd
                    className="px-2 py-1 rounded text-xs font-mono"
                    style={{
                      background: 'hsl(var(--harbor-surface-1))',
                      border: '1px solid hsl(var(--harbor-border-subtle))',
                      color: 'hsl(var(--harbor-text-secondary))',
                    }}
                  >
                    {formatKey(shortcut)}
                  </kbd>
                </div>
              ))}
            </div>
          </div>
        </div>

        {/* Footer */}
        <div
          className="px-6 py-3 border-t text-center"
          style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
        >
          <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
            Press{' '}
            <kbd className="px-1 rounded" style={{ background: 'hsl(var(--harbor-surface-1))' }}>
              Esc
            </kbd>{' '}
            to close
          </p>
        </div>
      </div>
    </div>
  );
}
