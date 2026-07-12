import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { VerifiedRelayName } from './VerifiedRelayName';

describe('VerifiedRelayName', () => {
  it('marks only verified claims', () => {
    const { rerender } = render(
      <VerifiedRelayName
        name={{ label: '@alice@relay', qualifiedName: '@alice@relay', trust: 'verified' }}
      />,
    );
    expect(screen.getByLabelText('Relay verified')).toBeInTheDocument();
    rerender(
      <VerifiedRelayName name={{ label: 'Alice', qualifiedName: null, trust: 'unverified' }} />,
    );
    expect(screen.queryByLabelText('Relay verified')).not.toBeInTheDocument();
    expect(screen.getByText('legacy name')).toBeInTheDocument();
  });
});
