import type { Card, SwipeAction, User } from '$lib/types';

export type ApiError = Error & { status?: number };

function apiError(message: string, status?: number): ApiError {
  const err = new Error(message) as ApiError;
  err.status = status;
  return err;
}

async function apiFetch<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  if (init.body) headers.set('content-type', 'application/json');

  const res = await fetch(path, {
    ...init,
    credentials: 'include',
    headers,
  });

  if (!res.ok) {
    const text = await res.text().catch(() => '');
    throw apiError(text || res.statusText, res.status);
  }

  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

export type MeResponse = {
  id: number;
  username: string;
  is_admin: boolean;
  must_change_password: boolean;
};
export type UserSummary = { id: number; username: string; is_admin: boolean };

export type SettingsResponse = {
  show_last_name: boolean;
  last_name: string;
  best_pick_mode: boolean;
};
export type PatchSettingsRequest = Partial<SettingsResponse>;

export type FeedQuery = {
  countries: string;
  pop: string;
  gender: string;
};

export type FeedBatchQuery = FeedQuery & { limit: number };

export type SwipeCreateRequest = {
  card_key: string;
  card_kind: 'group' | 'concept';
  action: SwipeAction;
};

export type SwipeCreateResponse = { status: string; match_created: boolean };
export type SwipeUndoResponse = {
  status: string;
  undone: null | { card_key: string; action: SwipeAction };
  match_removed: boolean;
};

export type CardRef = { card_key: string; card_kind: 'group' | 'concept'; display_name: string };

export type AdminUserRow = {
  id: number;
  username: string;
  is_admin: boolean;
  must_change_password: boolean;
  created_at: number;
  updated_at: number;
  deleted_at?: number | null;
};

export const api = {
  async me(): Promise<User> {
    const me = await apiFetch<MeResponse>('/api/me');
    return {
      id: me.id,
      username: me.username,
      is_admin: me.is_admin,
      must_change_password: me.must_change_password,
    };
  },

  async users(): Promise<User[]> {
    const users = await apiFetch<UserSummary[]>('/api/users');
    return users.map((u) => ({
      id: u.id,
      username: u.username,
      is_admin: u.is_admin,
      must_change_password: false,
    }));
  },

  async login(username: string, password: string): Promise<User> {
    const me = await apiFetch<MeResponse>('/api/auth/login', {
      method: 'POST',
      body: JSON.stringify({ username, password }),
    });
    return {
      id: me.id,
      username: me.username,
      is_admin: me.is_admin,
      must_change_password: me.must_change_password,
    };
  },

  async logout(): Promise<void> {
    await apiFetch<void>('/api/auth/logout', { method: 'POST' });
  },

  async changePassword(new_password: string): Promise<void> {
    await apiFetch<void>('/api/auth/change-password', {
      method: 'POST',
      body: JSON.stringify({ new_password }),
    });
  },

  async getSettings(): Promise<SettingsResponse> {
    return await apiFetch<SettingsResponse>('/api/settings');
  },

  async patchSettings(patch: PatchSettingsRequest): Promise<SettingsResponse> {
    return await apiFetch<SettingsResponse>('/api/settings', {
      method: 'PATCH',
      body: JSON.stringify(patch),
    });
  },

  async feedNext(q: FeedQuery): Promise<Card> {
    const params = new URLSearchParams({
      countries: q.countries,
      pop: q.pop,
      gender: q.gender,
    });
    return await apiFetch<Card>(`/api/feed/next?${params.toString()}`);
  },

  async feedBatch(q: FeedBatchQuery): Promise<Card[]> {
    const params = new URLSearchParams({
      countries: q.countries,
      pop: q.pop,
      gender: q.gender,
      limit: String(q.limit),
    });
    return await apiFetch<Card[]>(`/api/feed/batch?${params.toString()}`);
  },

  async cardDetails(
    card_key: string,
    opts?: { countries?: string; page?: number; page_size?: number }
  ): Promise<Card> {
    const params = new URLSearchParams();
    if (opts?.countries) params.set('countries', opts.countries);
    if (opts?.page !== undefined) params.set('page', String(opts.page));
    if (opts?.page_size !== undefined) params.set('page_size', String(opts.page_size));
    const qs = params.toString();
    return await apiFetch<Card>(`/api/card/${encodeURIComponent(card_key)}${qs ? `?${qs}` : ''}`);
  },

  async createSwipe(req: SwipeCreateRequest): Promise<SwipeCreateResponse> {
    return await apiFetch<SwipeCreateResponse>('/api/swipes', {
      method: 'POST',
      body: JSON.stringify({ ...req, action: req.action }),
    });
  },

  async undoSwipe(): Promise<SwipeUndoResponse> {
    return await apiFetch<SwipeUndoResponse>('/api/swipes/undo', { method: 'POST' });
  },

  async matches(): Promise<CardRef[]> {
    return await apiFetch<CardRef[]>('/api/matches');
  },

  async likes(): Promise<CardRef[]> {
    return await apiFetch<CardRef[]>('/api/likes');
  },

  async superlikes(): Promise<CardRef[]> {
    return await apiFetch<CardRef[]>('/api/superlikes');
  },

  async adminUsers(): Promise<AdminUserRow[]> {
    return await apiFetch<AdminUserRow[]>('/api/admin/users');
  },

  async adminCreateUser(req: {
    username: string;
    password: string;
    is_admin: boolean;
  }): Promise<AdminUserRow> {
    return await apiFetch<AdminUserRow>('/api/admin/users', {
      method: 'POST',
      body: JSON.stringify(req),
    });
  },

  async adminUpdateUser(
    id: number,
    patch: { username?: string; password?: string; is_admin?: boolean }
  ): Promise<AdminUserRow> {
    return await apiFetch<AdminUserRow>(`/api/admin/users/${id}`, {
      method: 'PATCH',
      body: JSON.stringify(patch),
    });
  },

  async adminDeleteUser(id: number): Promise<void> {
    await apiFetch<void>(`/api/admin/users/${id}`, { method: 'DELETE' });
  },
};
