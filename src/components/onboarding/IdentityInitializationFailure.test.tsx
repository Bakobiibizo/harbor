import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { IdentityInitializationFailure } from './IdentityInitializationFailure';

describe('IdentityInitializationFailure', () => {
  it('renders a retry action for recoverable registry failures', () => {
    const onRetry = vi.fn();
    render(
      <IdentityInitializationFailure
        state={{
          status: 'recoverableError',
          source: 'accountRegistry',
          error: {
            code: 'IO_ERROR',
            message: 'The account registry could not be read',
            recovery: 'Check access to the Harbor data directory and retry.',
          },
        }}
        onRetry={onRetry}
      />,
    );

    expect(screen.getByText("Harbor couldn't load your account")).toBeInTheDocument();
    expect(screen.getByText('The account registry could not be read')).toBeInTheDocument();
    expect(screen.queryByText('Create account')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    expect(onRetry).toHaveBeenCalledOnce();
  });

  it('renders corruption as a fatal stop without retry or account creation', () => {
    render(
      <IdentityInitializationFailure
        state={{
          status: 'fatalError',
          source: 'identityCorruption',
          error: {
            code: 'INVALID_DATA',
            message: 'Saved identity keys are inconsistent',
          },
        }}
        onRetry={vi.fn()}
      />,
    );

    expect(screen.getByText('Harbor cannot safely open this account')).toBeInTheDocument();
    expect(screen.getByText(/Do not create a replacement account/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Retry' })).not.toBeInTheDocument();
    expect(screen.queryByText('Create account')).not.toBeInTheDocument();
  });
});
