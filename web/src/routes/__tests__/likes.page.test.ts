import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { filters, settings } from '$lib/stores/app';

const { gotoMock, apiMock } = vi.hoisted(() => ({
  gotoMock: vi.fn(),
  apiMock: {
    likes: vi.fn(),
    superlikes: vi.fn(),
    cardDetails: vi.fn(),
    updateSwipe: vi.fn(),
  },
}));

vi.mock('$app/navigation', () => ({
  goto: gotoMock,
}));

vi.mock('$lib/api', () => ({
  api: apiMock,
}));

import LikesPage from '../likes/+page.svelte';

function mkDetails(cardKey: string, name: string) {
  return {
    card_key: cardKey,
    card_kind: 'group' as const,
    display_name: name,
    spellings_preview: [name],
    per_country: [
      { country: 'US' as const, ppm: 100, top100_any: true, gender: 'female' as const },
      { country: 'FR' as const, ppm: 90, top100_any: false, gender: 'female' as const },
      { country: 'CL' as const, ppm: 80, top100_any: false, gender: 'female' as const },
    ],
  };
}

describe('likes page', () => {
  beforeEach(() => {
    gotoMock.mockReset();
    apiMock.likes.mockReset();
    apiMock.superlikes.mockReset();
    apiMock.cardDetails.mockReset();
    apiMock.updateSwipe.mockReset();

    filters.set({ countries: ['US', 'FR', 'CL'], popularity: ['rare', 'common', 'top'], gender: 'any' });
    settings.set({ showLastName: true, lastName: 'Zarca Wong', bestPickMode: false });

    apiMock.likes.mockResolvedValue([
      {
        card_key: 'l1',
        card_kind: 'group',
        display_name: 'Lina',
        my_action: 'like',
        gender: 'female',
      },
    ]);
    apiMock.superlikes.mockResolvedValue([]);
    apiMock.cardDetails.mockResolvedValue(mkDetails('l1', 'Lina'));
    apiMock.updateSwipe.mockResolvedValue({ status: 'ok', match_created: false, match_removed: false });
  });

  it('loads likes by default and loads superlikes when segment changes', async () => {
    render(LikesPage);

    await waitFor(() => {
      expect(apiMock.likes).toHaveBeenCalled();
    });

    await fireEvent.click(screen.getByRole('button', { name: 'Super-likes' }));

    await waitFor(() => {
      expect(apiMock.superlikes).toHaveBeenCalled();
    });
  });

  it('switches to superlikes segment when updating a like to superlike', async () => {
    render(LikesPage);

    await fireEvent.click(await screen.findByRole('button', { name: /Lina/i }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Super-like' }));

    await waitFor(() => {
      expect(apiMock.updateSwipe).toHaveBeenCalledWith({ card_key: 'l1', action: 'superlike' });
      expect(apiMock.superlikes).toHaveBeenCalled();
    });
  });
});
