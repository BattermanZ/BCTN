import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { filters, settings } from '$lib/stores/app';

const { gotoMock, apiMock } = vi.hoisted(() => ({
  gotoMock: vi.fn(),
  apiMock: {
    matches: vi.fn(),
  },
}));

vi.mock('$app/navigation', () => ({
  goto: gotoMock,
}));

vi.mock('$lib/api', () => ({
  api: apiMock,
}));

import MatchesPage from '../matches/+page.svelte';

describe('matches page', () => {
  beforeEach(() => {
    gotoMock.mockReset();
    apiMock.matches.mockReset();

    filters.set({ countries: ['US', 'FR', 'CL'], popularity: ['rare', 'common', 'top'], gender: 'any' });
    settings.set({ showLastName: true, lastName: 'Zarca Wong', bestPickMode: false });
  });

  it('redirects to login when matches endpoint returns 401', async () => {
    apiMock.matches.mockRejectedValueOnce({ status: 401, message: 'Unauthorized' });

    render(MatchesPage);

    await waitFor(() => {
      expect(gotoMock).toHaveBeenCalledWith('/login');
    });
  });

  it('applies male/female filter with unisex included in both', async () => {
    apiMock.matches.mockResolvedValueOnce([
      { card_key: 'm1', card_kind: 'group', display_name: 'Tom', my_action: 'like', gender: 'male' },
      {
        card_key: 'm2',
        card_kind: 'group',
        display_name: 'Ana',
        my_action: 'like',
        gender: 'female',
      },
      {
        card_key: 'm3',
        card_kind: 'group',
        display_name: 'Alex',
        my_action: 'like',
        gender: 'unisex',
      },
    ]);

    render(MatchesPage);

    await screen.findByText('3 matches');
    await fireEvent.click(screen.getByRole('button', { name: 'Male' }));
    await screen.findByText('2 matches');

    await fireEvent.click(screen.getByRole('button', { name: 'Female' }));
    await screen.findByText('3 matches');
  });
});
