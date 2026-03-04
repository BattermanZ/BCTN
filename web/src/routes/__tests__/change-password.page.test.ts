import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { gotoMock, apiMock } = vi.hoisted(() => ({
  gotoMock: vi.fn(),
  apiMock: {
    me: vi.fn(),
    changePassword: vi.fn(),
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

import ChangePasswordPage from '../change-password/+page.svelte';
import { currentUser, filters, partnerUser, settings } from '$lib/stores/app';

describe('change-password page', () => {
  beforeEach(() => {
    gotoMock.mockReset();
    apiMock.me.mockReset();
    apiMock.changePassword.mockReset();
    apiMock.users.mockReset();
    apiMock.getSettings.mockReset();

    currentUser.set(null);
    partnerUser.set(null);
    filters.set({ countries: ['US', 'FR', 'CL'], popularity: ['rare', 'common', 'top'], gender: 'any' });
    settings.set({ showLastName: true, lastName: 'Zarca Wong', bestPickMode: false });

    apiMock.me.mockResolvedValue({
      id: 1,
      username: 'alice',
      is_admin: false,
      must_change_password: true,
    });
  });

  it('validates minimum password length before submitting', async () => {
    const { container } = render(ChangePasswordPage);

    const inputs = Array.from(container.querySelectorAll('input[type="password"]'));
    expect(inputs).toHaveLength(2);
    await fireEvent.input(inputs[0], { target: { value: 'short' } });
    await fireEvent.input(inputs[1], { target: { value: 'short' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Save password' }));

    expect(await screen.findByText('Password must be at least 8 characters.')).toBeTruthy();
    expect(apiMock.changePassword).not.toHaveBeenCalled();
  });

  it('changes password then hydrates stores and navigates home', async () => {
    apiMock.changePassword.mockResolvedValue(undefined);
    apiMock.me
      .mockResolvedValueOnce({ id: 1, username: 'alice', is_admin: false, must_change_password: true })
      .mockResolvedValueOnce({
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
      show_last_name: true,
      last_name: 'Zarca Wong',
      best_pick_mode: false,
      filter_countries: 'US,CL',
      filter_popularity: 'rare,top',
      filter_gender: 'any',
    });

    const { container } = render(ChangePasswordPage);

    const inputs = Array.from(container.querySelectorAll('input[type="password"]'));
    expect(inputs).toHaveLength(2);
    await fireEvent.input(inputs[0], { target: { value: 'longenough1' } });
    await fireEvent.input(inputs[1], { target: { value: 'longenough1' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Save password' }));

    await waitFor(() => {
      expect(apiMock.changePassword).toHaveBeenCalledWith('longenough1');
      expect(gotoMock).toHaveBeenCalledWith('/');
    });

    expect(get(currentUser)?.must_change_password).toBe(false);
    expect(get(partnerUser)?.username).toBe('bob');
    expect(get(filters)).toEqual({ countries: ['US', 'CL'], popularity: ['rare', 'top'], gender: 'any' });
  });
});
