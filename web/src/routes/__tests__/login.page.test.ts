import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { gotoMock, apiMock } = vi.hoisted(() => ({
  gotoMock: vi.fn(),
  apiMock: {
    login: vi.fn(),
    users: vi.fn(),
    getSettings: vi.fn(),
  },
}));

vi.mock('$app/navigation', () => ({
  goto: gotoMock,
}));

vi.mock('$lib/api', () => ({
  api: apiMock,
}));

import LoginPage from '../login/+page.svelte';
import { currentUser, filters, partnerUser, settings } from '$lib/stores/app';

describe('login page', () => {
  beforeEach(() => {
    gotoMock.mockReset();
    apiMock.login.mockReset();
    apiMock.users.mockReset();
    apiMock.getSettings.mockReset();
    currentUser.set(null);
    partnerUser.set(null);
    filters.set({ countries: ['US', 'FR', 'CL'], popularity: ['rare', 'common', 'top'], gender: 'any' });
    settings.set({ showLastName: true, lastName: 'Zarca Wong', bestPickMode: false });
  });

  it('redirects to change-password when backend requires it', async () => {
    apiMock.login.mockResolvedValueOnce({
      id: 1,
      username: 'alice',
      is_admin: false,
      must_change_password: true,
    });

    const { container } = render(LoginPage);

    const inputs = Array.from(container.querySelectorAll('input'));
    expect(inputs).toHaveLength(2);
    await fireEvent.input(inputs[0], { target: { value: ' alice ' } });
    await fireEvent.input(inputs[1], { target: { value: 'secret123' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));

    await waitFor(() => {
      expect(gotoMock).toHaveBeenCalledWith('/change-password');
    });
    expect(apiMock.users).not.toHaveBeenCalled();
    expect(apiMock.getSettings).not.toHaveBeenCalled();
  });

  it('hydrates stores and navigates home on successful login', async () => {
    apiMock.login.mockResolvedValueOnce({
      id: 1,
      username: 'alice',
      is_admin: false,
      must_change_password: false,
    });
    apiMock.users.mockResolvedValueOnce([
      { id: 1, username: 'alice', is_admin: false, must_change_password: false },
      { id: 2, username: 'bob', is_admin: false, must_change_password: false },
    ]);
    apiMock.getSettings.mockResolvedValueOnce({
      show_last_name: false,
      last_name: 'Doe',
      best_pick_mode: true,
      filter_countries: 'US,FR',
      filter_popularity: 'extra_rare,top',
      filter_gender: 'female',
    });

    const { container } = render(LoginPage);

    const inputs = Array.from(container.querySelectorAll('input'));
    expect(inputs).toHaveLength(2);
    await fireEvent.input(inputs[0], { target: { value: 'alice' } });
    await fireEvent.input(inputs[1], { target: { value: 'secret123' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));

    await waitFor(() => {
      expect(gotoMock).toHaveBeenCalledWith('/');
    });

    expect(get(currentUser)?.username).toBe('alice');
    expect(get(partnerUser)?.username).toBe('bob');
    expect(get(settings)).toEqual({ showLastName: false, lastName: 'Doe', bestPickMode: true });
    expect(get(filters)).toEqual({
      countries: ['US', 'FR'],
      popularity: ['extra_rare', 'top'],
      gender: 'female',
    });
  });
});
