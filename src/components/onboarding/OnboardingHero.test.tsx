import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it } from 'vitest';
import {
  HARBOR_DOCS_URL,
  HARBOR_QUICK_START_URL,
  OnboardingHero,
  onboardingDismissalKey,
} from './OnboardingHero';

describe('OnboardingHero', () => {
  beforeEach(() => localStorage.clear());

  it('shows a focused, accessible first-entry guide with working help links', () => {
    render(<OnboardingHero identityId="peer-alice" />);

    expect(screen.getByRole('dialog', { name: 'Social media you control' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Close getting started' })).toHaveFocus();
    expect(screen.getByRole('link', { name: 'Quick Start' })).toHaveAttribute(
      'href',
      HARBOR_QUICK_START_URL,
    );
    expect(screen.getByRole('link', { name: 'Browse Docs' })).toHaveAttribute(
      'href',
      HARBOR_DOCS_URL,
    );
  });

  it('persists dismissal for the current identity', () => {
    const { unmount } = render(<OnboardingHero identityId="peer-alice" />);
    fireEvent.click(screen.getByRole('button', { name: 'Start using Harbor' }));

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(localStorage.getItem(onboardingDismissalKey('peer-alice'))).toBe('1');

    unmount();
    render(<OnboardingHero identityId="peer-alice" />);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('keeps dismissal isolated between identities', () => {
    localStorage.setItem(onboardingDismissalKey('peer-alice'), '1');
    render(<OnboardingHero identityId="peer-bob" />);

    expect(screen.getByRole('dialog', { name: 'Social media you control' })).toBeInTheDocument();
  });

  it('closes and persists when Escape is pressed', () => {
    render(<OnboardingHero identityId="peer-alice" />);
    fireEvent.keyDown(window, { key: 'Escape' });

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(localStorage.getItem(onboardingDismissalKey('peer-alice'))).toBe('1');
  });
});
