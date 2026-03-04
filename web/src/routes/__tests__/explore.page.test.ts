import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { filters, settings } from '$lib/stores/app';

const { gotoMock, apiMock } = vi.hoisted(() => ({
  gotoMock: vi.fn(),
  apiMock: {
    exploreTop: vi.fn(),
    exploreSearch: vi.fn(),
    cardDetails: vi.fn(),
    createSwipe: vi.fn(),
    updateSwipe: vi.fn(),
  },
}));

vi.mock('$app/navigation', () => ({
  goto: gotoMock,
}));

vi.mock('$lib/api', () => ({
  api: apiMock,
}));

import ExplorePage from '../explore/+page.svelte';

function mkCardDetails(cardKey: string, displayName: string) {
  return {
    card_key: cardKey,
    card_kind: 'group' as const,
    display_name: displayName,
    spellings_preview: [displayName],
    per_country: [
      { country: 'US' as const, ppm: 100, top100_any: true, gender: 'female' as const },
      { country: 'FR' as const, ppm: 90, top100_any: false, gender: 'female' as const },
      { country: 'CL' as const, ppm: 80, top100_any: false, gender: 'female' as const },
    ],
  };
}

describe('explore page', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    gotoMock.mockReset();
    apiMock.exploreTop.mockReset();
    apiMock.exploreSearch.mockReset();
    apiMock.cardDetails.mockReset();
    apiMock.createSwipe.mockReset();
    apiMock.updateSwipe.mockReset();

    filters.set({ countries: ['US', 'FR', 'CL'], popularity: ['rare', 'common', 'top'], gender: 'any' });
    settings.set({ showLastName: true, lastName: 'Zarca Wong', bestPickMode: false });

    apiMock.exploreTop.mockResolvedValue([]);
    apiMock.exploreSearch.mockResolvedValue([]);
    apiMock.createSwipe.mockResolvedValue({ status: 'ok', match_created: false });
    apiMock.updateSwipe.mockResolvedValue({ status: 'ok', match_created: false, match_removed: false });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('loads top results on initial render with default filters', async () => {
    render(ExplorePage);

    await waitFor(() => {
      expect(apiMock.exploreTop).toHaveBeenCalledWith({
        countries: 'US,FR,CL',
        gender: 'any',
        page: 0,
        page_size: 50,
      });
    });
  });

  it('debounces and trims search query before requesting search API', async () => {
    render(ExplorePage);

    await fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    const input = screen.getByPlaceholderText('Search names…');

    await fireEvent.input(input, { target: { value: '  am  ' } });
    await vi.advanceTimersByTimeAsync(260);

    await waitFor(() => {
      expect(apiMock.exploreSearch).toHaveBeenCalledWith({
        q: 'am',
        countries: 'US,FR,CL',
        gender: 'any',
        page: 0,
        page_size: 50,
      });
    });
  });

  it('updates swipe when card was already swiped in explore sheet', async () => {
    apiMock.exploreTop.mockResolvedValueOnce([
      {
        card_key: 'c1',
        card_kind: 'group',
        display_name: 'Amelie',
        per_country: [],
        my_action: 'like',
        max_ppm: 42,
      },
    ]);
    apiMock.cardDetails.mockResolvedValueOnce(mkCardDetails('c1', 'Amelie'));
    apiMock.createSwipe.mockResolvedValueOnce({ status: 'already_swiped', match_created: false });

    render(ExplorePage);

    await fireEvent.click(await screen.findByRole('button', { name: /Amelie/i }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Super-like' }));

    await waitFor(() => {
      expect(apiMock.updateSwipe).toHaveBeenCalledWith({ card_key: 'c1', action: 'superlike' });
    });
  });
});
