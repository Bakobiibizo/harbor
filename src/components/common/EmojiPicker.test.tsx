import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { EmojiPicker } from './EmojiPicker';
import { activateProfile, suspendProfile } from '../../services/profileSession';

describe('EmojiPicker profile persistence', () => {
  beforeEach(() => {
    suspendProfile();
    localStorage.clear();
  });

  it('keeps recents and skin tone isolated across profiles', () => {
    activateProfile('profile-a');
    const first = render(<EmojiPicker onSelect={vi.fn()} onClose={vi.fn()} />);
    fireEvent.click(screen.getByTitle('Skin tone'));
    fireEvent.click(screen.getByTitle('Medium'));
    fireEvent.click(screen.getByTitle('grinning face'));
    expect(screen.getByTitle('Recently Used')).toBeInTheDocument();
    expect(localStorage.getItem('harbor:profile:profile-a:emoji-recents:v1')).toBe('["😀"]');
    expect(localStorage.getItem('harbor:profile:profile-a:emoji-skin-tone:v1')).toBe('3');
    first.unmount();

    suspendProfile();
    activateProfile('profile-b');
    const second = render(<EmojiPicker onSelect={vi.fn()} onClose={vi.fn()} />);
    expect(screen.queryByTitle('Recently Used')).not.toBeInTheDocument();
    expect(screen.getByTitle('Skin tone')).toHaveTextContent('👋');
    second.unmount();

    suspendProfile();
    activateProfile('profile-a');
    render(<EmojiPicker onSelect={vi.fn()} onClose={vi.fn()} />);
    expect(screen.getByTitle('Recently Used')).toBeInTheDocument();
    expect(screen.getByTitle('Skin tone')).toHaveTextContent('👋🏽');
  });

  it('migrates legacy emoji values only into the active profile', () => {
    localStorage.setItem('harbor-recent-emojis', '["😀"]');
    localStorage.setItem('harbor-emoji-skin-tone', '2');
    activateProfile('profile-a');

    const first = render(<EmojiPicker onSelect={vi.fn()} onClose={vi.fn()} />);
    expect(screen.getByTitle('Recently Used')).toBeInTheDocument();
    expect(screen.getByTitle('Skin tone')).toHaveTextContent('👋🏼');
    expect(localStorage.getItem('harbor-recent-emojis')).toBeNull();
    expect(localStorage.getItem('harbor-emoji-skin-tone')).toBeNull();
    first.unmount();

    suspendProfile();
    activateProfile('profile-b');
    render(<EmojiPicker onSelect={vi.fn()} onClose={vi.fn()} />);
    expect(screen.queryByTitle('Recently Used')).not.toBeInTheDocument();
    expect(screen.getByTitle('Skin tone')).toHaveTextContent('👋');
  });
});
