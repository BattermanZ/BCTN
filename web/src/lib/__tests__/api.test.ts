import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { api } from '$lib/api';

type MockResponseInit = {
  ok: boolean;
  status: number;
  statusText?: string;
  json?: unknown;
  text?: string;
};

function mockResponse(init: MockResponseInit): Response {
  return {
    ok: init.ok,
    status: init.status,
    statusText: init.statusText ?? '',
    json: vi.fn(async () => init.json),
    text: vi.fn(async () => init.text ?? ''),
  } as unknown as Response;
}

describe('api', () => {
  const fetchMock = vi.fn();

  beforeEach(() => {
    fetchMock.mockReset();
    vi.stubGlobal('fetch', fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('sends login request with credentials and JSON content type', async () => {
    fetchMock.mockResolvedValueOnce(
      mockResponse({
        ok: true,
        status: 200,
        json: { id: 1, username: 'alice', is_admin: false, must_change_password: false },
      })
    );

    const me = await api.login('alice', 'secret');

    expect(me).toEqual({ id: 1, username: 'alice', is_admin: false, must_change_password: false });
    expect(fetchMock).toHaveBeenCalledTimes(1);

    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe('/api/auth/login');
    expect(init.method).toBe('POST');
    expect(init.credentials).toBe('include');
    expect(init.body).toBe(JSON.stringify({ username: 'alice', password: 'secret' }));
    expect(new Headers(init.headers).get('content-type')).toBe('application/json');
  });

  it('encodes feed batch query params', async () => {
    fetchMock.mockResolvedValueOnce(mockResponse({ ok: true, status: 200, json: [] }));

    await api.feedBatch({ countries: 'US,FR', pop: 'rare,top', gender: 'any', limit: 7 });

    const [url] = fetchMock.mock.calls[0] as [string];
    expect(url).toContain('/api/feed/batch?');
    expect(url).toContain('countries=US%2CFR');
    expect(url).toContain('pop=rare%2Ctop');
    expect(url).toContain('gender=any');
    expect(url).toContain('limit=7');
  });

  it('returns void for 204 endpoints', async () => {
    fetchMock.mockResolvedValueOnce(mockResponse({ ok: true, status: 204 }));

    await expect(api.logout()).resolves.toBeUndefined();
  });

  it('throws an ApiError with status and response text for failed requests', async () => {
    fetchMock.mockResolvedValueOnce(
      mockResponse({ ok: false, status: 401, statusText: 'Unauthorized', text: 'No session' })
    );

    await expect(api.me()).rejects.toMatchObject({ message: 'No session', status: 401 });
  });
});
