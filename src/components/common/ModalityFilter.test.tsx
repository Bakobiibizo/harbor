import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ModalityFilter } from './ModalityFilter';

describe('ModalityFilter', () => {
  it('exposes compact pressed-state controls with All as the user-facing label', () => {
    const onChange = vi.fn();
    render(<ModalityFilter value="all" onChange={onChange} label="Filter test posts" />);

    expect(screen.getByRole('group', { name: 'Filter test posts' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'All' })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.queryByRole('button', { name: 'Posts' })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Video' }));
    expect(onChange).toHaveBeenCalledWith('videos');
  });
});
