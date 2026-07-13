import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const designSystem = readFileSync('src/styles/design-system.css', 'utf8');

describe('shared interaction motion', () => {
  it('defines theme-driven hover, pressed, focus, selected, disabled and loading feedback', () => {
    expect(designSystem).toContain('--harbor-interaction-hover-filter');
    expect(designSystem).toContain('--harbor-interaction-pressed-filter');
    expect(designSystem).toContain('--harbor-interaction-selected-ring');
    expect(designSystem).toContain('--harbor-interaction-focus-ring');
    expect(designSystem).toContain('--harbor-interaction-disabled-opacity');
    expect(designSystem).toContain('--harbor-interaction-loading-opacity');
    expect(designSystem).toContain("[aria-current='page']");
    expect(designSystem).toContain("[aria-busy='true']");
  });

  it('limits shared transitions to paint-only properties and honors reduced motion', () => {
    expect(designSystem).toContain(
      'transition-property: color, background-color, border-color, box-shadow, filter, opacity;',
    );
    expect(designSystem).toContain('@media (prefers-reduced-motion: reduce)');
    expect(designSystem).toContain('transition-duration: 0.01ms !important');
    expect(designSystem).toContain('animation-iteration-count: 1 !important');
  });
});
