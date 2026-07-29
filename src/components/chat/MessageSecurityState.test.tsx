import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { MessageSecurityState } from './MessageSecurityState';

describe('MessageSecurityState', () => {
  it.each([
    ['tampered', 'Message authenticity check failed'],
    ['wrong_key', 'Message key no longer matches'],
    ['corrupt_payload', 'Message data is incomplete'],
  ] as const)('renders privacy-safe guidance for %s', (kind, title) => {
    render(<MessageSecurityState state={{ kind }} />);

    expect(screen.getByRole('alert')).toHaveTextContent(title);
    expect(screen.getByRole('alert')).toHaveTextContent('resend');
  });

  it('identifies an unsupported format without exposing message material', () => {
    render(<MessageSecurityState state={{ kind: 'unsupported_version', version: 7 }} />);

    expect(screen.getByRole('alert')).toHaveTextContent('Update Harbor');
    expect(screen.getByRole('alert')).toHaveTextContent('encryption format 7');
    expect(screen.getByRole('alert')).not.toHaveTextContent(/cipher|peer|message id/i);
  });
});
