import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const { gotoMock, apiMock } = vi.hoisted(() => ({
  gotoMock: vi.fn(),
  apiMock: {
    patchSettings: vi.fn(),
    logout: vi.fn(),
  },
}));

vi.mock('$app/navigation', () => ({
  goto: gotoMock,
}));

vi.mock('$app/paths', () => ({
  resolveRoute: (x: string) => x,
}));

vi.mock('$lib/api', () => ({
  api: apiMock,
}));

import SettingsPage from '../settings/+page.svelte';
import { currentUser, filters, partnerUser, settings } from '$lib/stores/app';

describe('settings page', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    gotoMock.mockReset();
    apiMock.patchSettings.mockReset();
    apiMock.logout.mockReset();

    currentUser.set({ id: 1, username: 'alice', is_admin: false, must_change_password: false });
    partnerUser.set({ id: 2, username: 'bob', is_admin: false, must_change_password: false });
    filters.set({ countries: ['US'], popularity: ['rare', 'common', 'top'], gender: 'any' });
    settings.set({ showLastName: true, lastName: 'Zarca Wong', bestPickMode: false });

    apiMock.patchSettings.mockImplementation(async (patch) => ({
      show_last_name: patch.show_last_name ?? true,
      last_name: patch.last_name ?? 'Zarca Wong',
      best_pick_mode: patch.best_pick_mode ?? false,
      filter_countries: patch.filter_countries ?? get(filters).countries.join(','),
      filter_popularity: patch.filter_popularity ?? get(filters).popularity.join(','),
      filter_gender: patch.filter_gender ?? get(filters).gender,
    }));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('does not allow clearing the final selected country', async () => {
    render(SettingsPage);

    await Promise.resolve();
    await fireEvent.click(screen.getByRole('button', { name: 'United States' }));

    expect(get(filters).countries).toEqual(['US']);
    vi.advanceTimersByTime(500);
    expect(apiMock.patchSettings).not.toHaveBeenCalledWith(
      expect.objectContaining({ filter_countries: '' })
    );
  });

  it('debounces filter saves and sends the final payload', async () => {
    render(SettingsPage);

    await Promise.resolve();
    await fireEvent.click(screen.getByRole('button', { name: 'France' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Chile' }));

    vi.advanceTimersByTime(350);

    await waitFor(() => {
      expect(apiMock.patchSettings).toHaveBeenCalledTimes(1);
    });

    expect(apiMock.patchSettings).toHaveBeenCalledWith(
      expect.objectContaining({
        filter_countries: 'US,FR,CL',
        filter_popularity: 'rare,common,top',
        filter_gender: 'any',
      })
    );
  });
});
