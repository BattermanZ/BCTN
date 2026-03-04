import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { currentUser, partnerUser } from '$lib/stores/app';

const { gotoMock, apiMock } = vi.hoisted(() => ({
  gotoMock: vi.fn(),
  apiMock: {
    me: vi.fn(),
    adminUsers: vi.fn(),
    adminCreateUser: vi.fn(),
    adminUpdateUser: vi.fn(),
    adminDeleteUser: vi.fn(),
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
    apiMock.adminUpdateUser.mockReset();
    apiMock.adminDeleteUser.mockReset();
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
    apiMock.adminUpdateUser.mockResolvedValue({
      id: 1,
      username: 'alice-updated',
      is_admin: false,
      must_change_password: false,
      created_at: 0,
      updated_at: 1,
      deleted_at: null,
    });
    apiMock.adminDeleteUser.mockResolvedValue(undefined);
    vi.stubGlobal('confirm', vi.fn(() => true));
  });

  afterEach(() => {
    vi.unstubAllGlobals();
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

  it('edits a user and refreshes partner user list', async () => {
    currentUser.set({ id: 2, username: 'bob', is_admin: true, must_change_password: false });
    apiMock.adminUsers.mockResolvedValueOnce([
      {
        id: 1,
        username: 'alice',
        is_admin: false,
        must_change_password: false,
        created_at: 0,
        updated_at: 0,
        deleted_at: null,
      },
      {
        id: 2,
        username: 'bob',
        is_admin: true,
        must_change_password: false,
        created_at: 0,
        updated_at: 0,
        deleted_at: null,
      },
    ]);
    apiMock.adminUsers.mockResolvedValueOnce([
      {
        id: 1,
        username: 'alice-updated',
        is_admin: false,
        must_change_password: false,
        created_at: 0,
        updated_at: 1,
        deleted_at: null,
      },
      {
        id: 2,
        username: 'bob',
        is_admin: true,
        must_change_password: false,
        created_at: 0,
        updated_at: 0,
        deleted_at: null,
      },
    ]);
    apiMock.users.mockResolvedValueOnce([
      { id: 2, username: 'bob', is_admin: true, must_change_password: false },
      { id: 1, username: 'alice-updated', is_admin: false, must_change_password: false },
    ]);

    const { container } = render(AdminPage);

    await screen.findByText('alice');
    await screen.findByText('bob');

    const aliceCard = screen.getByText('alice').closest('.rounded-2xl') as HTMLElement | null;
    expect(aliceCard).toBeTruthy();
    await fireEvent.click(within(aliceCard!).getByRole('button', { name: 'Edit' }));

    const usernameInput = container.querySelector('input') as HTMLInputElement | null;
    const passwordInput = container.querySelector('input[type="password"]') as HTMLInputElement | null;
    const adminCheckbox = container.querySelector('input[type="checkbox"]') as HTMLInputElement | null;

    expect(usernameInput).toBeTruthy();
    expect(passwordInput).toBeTruthy();
    expect(adminCheckbox).toBeTruthy();

    await fireEvent.input(usernameInput!, { target: { value: '  alice-updated  ' } });
    await fireEvent.input(passwordInput!, { target: { value: 'newtemp123' } });
    await fireEvent.click(adminCheckbox!);
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(apiMock.adminUpdateUser).toHaveBeenCalledWith(1, {
        username: 'alice-updated',
        password: 'newtemp123',
        is_admin: true,
      });
    });
  });

  it('deletes a user after confirmation and refreshes partner user list', async () => {
    currentUser.set({ id: 2, username: 'bob', is_admin: true, must_change_password: false });
    apiMock.adminUsers.mockResolvedValueOnce([
      {
        id: 1,
        username: 'alice',
        is_admin: false,
        must_change_password: false,
        created_at: 0,
        updated_at: 0,
        deleted_at: null,
      },
      {
        id: 2,
        username: 'bob',
        is_admin: true,
        must_change_password: false,
        created_at: 0,
        updated_at: 0,
        deleted_at: null,
      },
    ]);
    apiMock.adminUsers.mockResolvedValueOnce([
      {
        id: 2,
        username: 'bob',
        is_admin: true,
        must_change_password: false,
        created_at: 0,
        updated_at: 0,
        deleted_at: null,
      },
    ]);
    apiMock.users.mockResolvedValueOnce([
      { id: 2, username: 'bob', is_admin: true, must_change_password: false },
    ]);

    render(AdminPage);

    await screen.findByText('alice');
    await screen.findByText('bob');

    const aliceCard = screen.getByText('alice').closest('.rounded-2xl') as HTMLElement | null;
    expect(aliceCard).toBeTruthy();
    await fireEvent.click(within(aliceCard!).getByRole('button', { name: 'Delete' }));

    await waitFor(() => {
      expect(apiMock.adminDeleteUser).toHaveBeenCalledWith(1);
      expect(apiMock.adminUsers).toHaveBeenCalledTimes(2);
    });
    expect(get(partnerUser)).toBeNull();
  });
});
