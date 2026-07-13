import { useEffect, useRef, useState } from 'react';
import { HarborIcon, XIcon } from '../icons';

const ONBOARDING_DISMISSAL_PREFIX = 'harbor-onboarding-guide-dismissed-v1:';

export const HARBOR_DOCS_URL = 'https://www.social-harbor.com/docs/';
export const HARBOR_QUICK_START_URL = 'https://www.social-harbor.com/docs/#before';

export function onboardingDismissalKey(identityId: string): string {
  return `${ONBOARDING_DISMISSAL_PREFIX}${encodeURIComponent(identityId)}`;
}

export function hasDismissedOnboarding(identityId: string): boolean {
  try {
    return localStorage.getItem(onboardingDismissalKey(identityId)) === '1';
  } catch {
    return false;
  }
}

function persistOnboardingDismissal(identityId: string): void {
  try {
    localStorage.setItem(onboardingDismissalKey(identityId), '1');
  } catch {
    // A locked-down webview may reject storage. The guide can still be dismissed for this session.
  }
}

interface OnboardingHeroProps {
  identityId: string;
}

export function OnboardingHero({ identityId }: OnboardingHeroProps) {
  const [isOpen, setIsOpen] = useState(() => !hasDismissedOnboarding(identityId));
  const closeButtonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    setIsOpen(!hasDismissedOnboarding(identityId));
  }, [identityId]);

  useEffect(() => {
    if (!isOpen) return;

    closeButtonRef.current?.focus();
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        persistOnboardingDismissal(identityId);
        setIsOpen(false);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [identityId, isOpen]);

  if (!isOpen) return null;

  const dismiss = () => {
    persistOnboardingDismissal(identityId);
    setIsOpen(false);
  };

  return (
    <div
      className="fixed inset-0 z-[190] flex items-center justify-center p-5"
      style={{ background: 'hsl(var(--harbor-brand-navy) / 0.78)' }}
    >
      <section
        role="dialog"
        aria-modal="true"
        aria-labelledby="harbor-onboarding-title"
        aria-describedby="harbor-onboarding-description"
        className="animate-fade-in-scale relative w-full max-w-xl overflow-hidden rounded-2xl p-8"
        style={{
          background: 'hsl(var(--harbor-bg-elevated))',
          border: '1px solid hsl(var(--harbor-border-subtle))',
          boxShadow: 'var(--shadow-xl)',
        }}
      >
        <button
          ref={closeButtonRef}
          type="button"
          onClick={dismiss}
          aria-label="Close getting started"
          className="harbor-interactive absolute right-4 top-4 rounded-lg p-2"
          style={{ color: 'hsl(var(--harbor-text-secondary))' }}
        >
          <XIcon className="h-5 w-5" />
        </button>

        <HarborIcon className="h-16 w-16" />
        <p
          className="mt-6 text-sm font-semibold uppercase tracking-[0.18em]"
          style={{ color: 'hsl(var(--harbor-primary))' }}
        >
          Welcome to Harbor
        </p>
        <h1
          id="harbor-onboarding-title"
          className="mt-2 text-3xl font-bold text-balance"
          style={{ color: 'hsl(var(--harbor-text-primary))' }}
        >
          Social media you control
        </h1>
        <p
          id="harbor-onboarding-description"
          className="mt-4 max-w-lg text-sm leading-6"
          style={{ color: 'hsl(var(--harbor-text-secondary))' }}
        >
          Start with the quick guide to connect with someone, choose who can see your posts, and
          learn how Harbor keeps your identity on your device.
        </p>

        <div className="mt-7 flex flex-wrap items-center gap-3">
          <a
            href={HARBOR_QUICK_START_URL}
            target="_blank"
            rel="noopener noreferrer"
            className="harbor-button harbor-button--primary harbor-interactive inline-flex items-center justify-center rounded-lg px-5 py-2.5 text-sm font-semibold"
          >
            Quick Start
          </a>
          <a
            href={HARBOR_DOCS_URL}
            target="_blank"
            rel="noopener noreferrer"
            className="harbor-button harbor-button--secondary harbor-interactive inline-flex items-center justify-center rounded-lg px-5 py-2.5 text-sm font-semibold"
          >
            Browse Docs
          </a>
          <button
            type="button"
            onClick={dismiss}
            className="harbor-interactive ml-auto rounded-lg px-3 py-2 text-sm font-medium"
            style={{ color: 'hsl(var(--harbor-text-secondary))' }}
          >
            Start using Harbor
          </button>
        </div>
      </section>
    </div>
  );
}
