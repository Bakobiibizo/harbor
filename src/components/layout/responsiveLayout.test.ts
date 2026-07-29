import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const appStyles = readFileSync('src/App.css', 'utf8');

describe('responsive application layout', () => {
  it('defines wide, desktop, compact, and narrow shell widths', () => {
    expect(appStyles).toContain('width: 18rem');
    expect(appStyles).toContain('@media (max-width: 1279px)');
    expect(appStyles).toContain('width: 15rem');
    expect(appStyles).toContain('@media (max-width: 1023px)');
    expect(appStyles).toContain('width: 5rem');
    expect(appStyles).toContain('@media (max-width: 767px)');
    expect(appStyles).toContain('width: 4.5rem');
  });

  it('collapses utility controls for compact widths and short windows', () => {
    expect(appStyles).toContain('@media (min-width: 1024px) and (max-height: 759px)');
    expect(appStyles).toMatch(/\.harbor-sidebar-compact-utilities\s*\{[\s\S]*?display:\s*block;/);
  });

  it('uses the content pane for settings and community-board reflow', () => {
    expect(appStyles).toContain('container-name: harbor-content');
    expect(appStyles).toContain('@container harbor-content (max-width: 46rem)');
    expect(appStyles).toContain('.harbor-settings-layout');
    expect(appStyles).toContain('.harbor-boards-content');
  });
});
