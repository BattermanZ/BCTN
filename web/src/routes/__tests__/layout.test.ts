import { render, waitFor } from '@testing-library/svelte';
import { readable } from 'svelte/store';
import { get } from 'svelte/store';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { gotoMock, apiMock } = vi.hoisted(() => ({
  gotoMock: vi.fn(),
  apiMock: {
    me: vi.fn(),
    users: vi.fn(),
    getSettings: vi.fn(),
  },
}));

vi.mock('$app/navigation', () => ({
  goto: gotoMock,
}));

vi.mock('$app/stores', () => ({
  page: readable({ url: new URL('http://localhost/') }),
}));

vi.mock('$lib/api', () => ({
  api: apiMock,
}));

import LayoutHost from './fixtures/LayoutHost.svelte';
import { currentUser, partnerUser } from '$lib/stores/app';

describe('layout bootstrap', () => {
  beforeEach(() => {
    gotoMock.mockReset();
    apiMock.me.mockReset();
    apiMock.users.mockReset();
    apiMock.getSettings.mockReset();
    currentUser.set(null);
    partnerUser.set(null);
  });

  it('redirects to login on unauthenticated bootstrap', async () => {
    apiMock.me.mockRejectedValueOnce({ status: 401, message: 'Unauthorized' });

    render(LayoutHost);

    await waitFor(() => {
      expect(gotoMock).toHaveBeenCalledWith('/login');
    });
    expect(get(currentUser)).toBeNull();
    expect(get(partnerUser)).toBeNull();
  });

  it('redirects to change-password when current user must rotate password', async () => {
    apiMock.me.mockResolvedValueOnce({
      id: 1,
      username: 'alice',
      is_admin: false,
      must_change_password: true,
    });

    render(LayoutHost);

    await waitFor(() => {
      expect(gotoMock).toHaveBeenCalledWith('/change-password');
    });
    expect(get(partnerUser)).toBeNull();
  });
});
