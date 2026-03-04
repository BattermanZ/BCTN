import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { currentUser, partnerUser } from '$lib/stores/app';

const { gotoMock, apiMock } = vi.hoisted(() => ({
  gotoMock: vi.fn(),
  apiMock: {
    me: vi.fn(),
    adminUsers: vi.fn(),
    adminCreateUser: vi.fn(),
    users: vi.fn(),
  },
}));

vi.mock('$app/navigation', () => ({
  goto: gotoMock,
}));

vi.mock('$lib/api', () => ({
  api: apiMock,
}));

import AdminPage from '../admin/+page.svelte';

describe('admin page', () => {
  beforeEach(() => {
    gotoMock.mockReset();
    apiMock.me.mockReset();
    apiMock.adminUsers.mockReset();
    apiMock.adminCreateUser.mockReset();
    apiMock.users.mockReset();

    currentUser.set(null);
    partnerUser.set(null);

    apiMock.adminUsers.mockResolvedValue([]);
    apiMock.adminCreateUser.mockResolvedValue({
      id: 3,
      username: 'newuser',
      is_admin: false,
      must_change_password: true,
      created_at: 0,
      updated_at: 0,
      deleted_at: null,
    });
  });

  it('redirects to login when no user is available during bootstrap', async () => {
    apiMock.me.mockRejectedValueOnce(new Error('no session'));

    render(AdminPage);

    await waitFor(() => {
      expect(gotoMock).toHaveBeenCalledWith('/login');
    });
  });

  it('creates a user and refreshes partner user list', async () => {
    currentUser.set({ id: 1, username: 'alice', is_admin: true, must_change_password: false });
    apiMock.adminUsers.mockResolvedValueOnce([]).mockResolvedValueOnce([
      {
        id: 1,
        username: 'alice',
        is_admin: true,
        must_change_password: false,
        created_at: 0,
        updated_at: 0,
        deleted_at: null,
      },
      {
        id: 3,
        username: 'newuser',
        is_admin: false,
        must_change_password: true,
        created_at: 0,
        updated_at: 0,
        deleted_at: null,
      },
    ]);
    apiMock.users.mockResolvedValueOnce([
      { id: 1, username: 'alice', is_admin: true, must_change_password: false },
      { id: 3, username: 'newuser', is_admin: false, must_change_password: false },
    ]);

    const { container } = render(AdminPage);

    await waitFor(() => {
      expect(apiMock.adminUsers).toHaveBeenCalledTimes(1);
    });

    const inputs = Array.from(container.querySelectorAll('input'));
    const usernameInput = inputs.find((x) => x.getAttribute('placeholder') === 'Username');
    const passwordInput = inputs.find((x) => x.getAttribute('type') === 'password');

    expect(usernameInput).toBeTruthy();
    expect(passwordInput).toBeTruthy();

    await fireEvent.input(usernameInput!, { target: { value: '  newuser  ' } });
    await fireEvent.input(passwordInput!, { target: { value: 'temporary1' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Create' }));

    await waitFor(() => {
      expect(apiMock.adminCreateUser).toHaveBeenCalledWith({
        username: 'newuser',
        password: 'temporary1',
        is_admin: false,
      });
      expect(apiMock.adminUsers).toHaveBeenCalledTimes(2);
    });

    expect(get(partnerUser)?.username).toBe('newuser');
  });
});
