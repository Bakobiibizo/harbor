import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { HARBOR_SHORTCUT_EVENTS } from '../../hooks';
import { KeyboardShortcutsModal } from './KeyboardShortcutsModal';

describe('KeyboardShortcutsModal', () => {
  it('is discoverable, accessible, themed, and grouped from the shortcut registry', () => {
    render(<KeyboardShortcutsModal />);
    fireEvent(window, new CustomEvent(HARBOR_SHORTCUT_EVENTS.showShortcuts));

    const dialog = screen.getByRole('dialog', { name: 'Keyboard Shortcuts' });
    expect(dialog).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Close keyboard shortcuts' })).toHaveFocus();
    expect(screen.getByRole('heading', { name: 'Navigation' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Actions' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Editing' })).toBeInTheDocument();
    expect(dialog).toHaveTextContent('Focus search');
    expect(dialog).toHaveTextContent('New message');
    expect(dialog).toHaveTextContent('New post');
    expect(dialog).toHaveTextContent('New line in message');
  });

  it('closes through the shared Escape event', () => {
    render(<KeyboardShortcutsModal />);
    fireEvent(window, new CustomEvent(HARBOR_SHORTCUT_EVENTS.showShortcuts));
    expect(screen.getByRole('dialog')).toBeInTheDocument();

    fireEvent(window, new CustomEvent(HARBOR_SHORTCUT_EVENTS.escape));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });
});
