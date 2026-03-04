import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const { gotoMock, apiMock } = vi.hoisted(() => ({
  gotoMock: vi.fn(),
  apiMock: {
    feedBatch: vi.fn(),
    cardDetails: vi.fn(),
    createSwipe: vi.fn(),
    undoSwipe: vi.fn(),
  },
}));

vi.mock('$app/navigation', () => ({
  goto: gotoMock,
}));

vi.mock('$lib/api', () => ({
  api: apiMock,
}));

import SwipeDeck from '../SwipeDeck.svelte';
import { filters, settings, swipes } from '$lib/stores/app';
import type { Card } from '$lib/types';

function mkCard(i: number): Card {
  return {
    card_key: `card-${i}`,
    card_kind: 'group',
    display_name: `Name ${i}`,
    spellings_preview: [`Name${i}`],
    per_country: [
      { country: 'US', ppm: 100, top100_any: true, gender: 'female' },
      { country: 'FR', ppm: 90, top100_any: false, gender: 'female' },
      { country: 'CL', ppm: 80, top100_any: false, gender: 'female' },
    ],
  };
}

describe('SwipeDeck', () => {
  beforeEach(() => {
    vi.useFakeTimers({
      toFake: [
        'setTimeout',
        'clearTimeout',
        'setInterval',
        'clearInterval',
        'Date',
        'performance',
        'requestAnimationFrame',
        'cancelAnimationFrame',
      ],
    });
    gotoMock.mockReset();
    apiMock.feedBatch.mockReset();
    apiMock.cardDetails.mockReset();
    apiMock.createSwipe.mockReset();
    apiMock.undoSwipe.mockReset();

    filters.set({ countries: ['US', 'FR', 'CL'], popularity: ['rare', 'common', 'top'], gender: 'any' });
    settings.set({ showLastName: true, lastName: 'Zarca Wong', bestPickMode: false });
    swipes.set({ byCardKey: {} });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('redirects to login when feed loading returns 401', async () => {
    apiMock.feedBatch.mockRejectedValueOnce({ status: 401, message: 'Unauthorized' });

    render(SwipeDeck);

    await waitFor(() => {
      expect(gotoMock).toHaveBeenCalledWith('/login');
    });
  });

  it('records swipe action and sends createSwipe for the top card', async () => {
    const firstBatch = Array.from({ length: 10 }, (_, i) => mkCard(i + 1));
    apiMock.feedBatch
      .mockResolvedValueOnce(firstBatch)
      .mockResolvedValueOnce([]);
    apiMock.createSwipe.mockResolvedValue({ status: 'ok', match_created: false });

    render(SwipeDeck);
    await waitFor(() => {
      expect(apiMock.feedBatch).toHaveBeenCalled();
    });

    await fireEvent.click(screen.getByRole('button', { name: 'Like' }));
    await vi.advanceTimersByTimeAsync(2000);

    await waitFor(() => {
      expect(apiMock.createSwipe).toHaveBeenCalledTimes(1);
    });
    const payload = apiMock.createSwipe.mock.calls[0][0];
    expect(payload.action).toBe('like');
    expect(payload.card_kind).toBe('group');
    expect(firstBatch.map((c) => c.card_key)).toContain(payload.card_key);
  });
});
