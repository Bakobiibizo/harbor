import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { LockAccountDialog } from './LockAccountDialog';

describe('LockAccountDialog', () => {
  it('warns about the local password requirement and recovery limit', () => {
    render(<LockAccountDialog isLocking={false} onCancel={vi.fn()} onConfirm={vi.fn()} />);

    expect(screen.getByRole('dialog', { name: 'Lock this account?' })).toHaveTextContent(
      'You will need your password to unlock this account on this device.',
    );
    expect(screen.getByRole('dialog')).toHaveTextContent(
      'Harbor cannot recover a forgotten password.',
    );
    expect(screen.getByRole('button', { name: 'Cancel' })).toHaveFocus();
  });

  it('cancels safely without locking', () => {
    const onCancel = vi.fn();
    const onConfirm = vi.fn();
    render(<LockAccountDialog isLocking={false} onCancel={onCancel} onConfirm={onConfirm} />);

    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onCancel).toHaveBeenCalledOnce();
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it('requires explicit confirmation and supports Escape', () => {
    const onCancel = vi.fn();
    const onConfirm = vi.fn();
    render(<LockAccountDialog isLocking={false} onCancel={onCancel} onConfirm={onConfirm} />);

    fireEvent.click(screen.getByRole('button', { name: 'Lock Account' }));
    expect(onConfirm).toHaveBeenCalledOnce();

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledOnce();
  });
});
