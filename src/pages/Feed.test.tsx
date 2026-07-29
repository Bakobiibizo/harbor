import { fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { describe, expect, it } from 'vitest';
import { FindContactsButton } from './Feed';

function renderFindContactsAction() {
  render(
    <MemoryRouter initialEntries={['/feed']}>
      <Routes>
        <Route path="/feed" element={<FindContactsButton />} />
        <Route path="/network" element={<h1>Network contacts</h1>} />
      </Routes>
    </MemoryRouter>,
  );
}

describe('Find Contacts feed action', () => {
  it('navigates to contact discovery when clicked', () => {
    renderFindContactsAction();

    fireEvent.click(screen.getByRole('button', { name: 'Find Contacts' }));

    expect(screen.getByRole('heading', { name: 'Network contacts' })).toBeInTheDocument();
  });

  it('is a native keyboard control and follows a keyboard-generated activation click', () => {
    renderFindContactsAction();
    const action = screen.getByRole('button', { name: 'Find Contacts' });
    action.focus();

    expect(action.tagName).toBe('BUTTON');
    expect(action).toHaveFocus();
    fireEvent.keyDown(action, { key: 'Enter' });
    fireEvent.click(action, { detail: 0 });

    expect(screen.getByRole('heading', { name: 'Network contacts' })).toBeInTheDocument();
  });
});
