import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Input } from './Input';

describe('Input', () => {
  it('should render an input element', () => {
    render(<Input />);

    expect(screen.getByRole('textbox')).toBeInTheDocument();
  });

  it('should render a label when provided', () => {
    render(<Input label="Username" />);

    expect(screen.getByText('Username')).toBeInTheDocument();
  });

  it('should not render a label when not provided', () => {
    render(<Input />);

    expect(screen.queryByText('Username')).not.toBeInTheDocument();
  });

  it('should show error message when error prop is provided', () => {
    render(<Input error="This field is required" />);

    expect(screen.getByText('This field is required')).toBeInTheDocument();
  });

  it('should apply error styling when error is present', () => {
    render(<Input error="Error" />);

    const input = screen.getByRole('textbox');
    expect(input.className).toContain('border-red-500');
  });

  it('should call onChange when text is entered', () => {
    const handleChange = vi.fn();
    render(<Input onChange={handleChange} />);

    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'hello' } });

    expect(handleChange).toHaveBeenCalled();
  });

  it('should render as disabled when disabled prop is true', () => {
    render(<Input disabled />);

    expect(screen.getByRole('textbox')).toBeDisabled();
  });

  it('should accept placeholder text', () => {
    render(<Input placeholder="Enter text..." />);

    expect(screen.getByPlaceholderText('Enter text...')).toBeInTheDocument();
  });

  it('should apply custom className', () => {
    render(<Input className="custom-input" />);

    const input = screen.getByRole('textbox');
    expect(input.className).toContain('custom-input');
  });

  it('should forward ref to input element', () => {
    const ref = vi.fn();
    render(<Input ref={ref} />);

    expect(ref).toHaveBeenCalled();
  });

  it('should forward additional HTML input attributes', () => {
    render(<Input type="email" aria-label="Email address" />);

    const input = screen.getByLabelText('Email address');
    expect(input).toHaveAttribute('type', 'email');
  });
});
