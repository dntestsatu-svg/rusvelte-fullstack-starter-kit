import { describe, expect, it, vi } from 'vitest';
import type { AuthUser } from '../api/auth';
import {
	buildLoginRedirectPath,
	DEFAULT_AUTHENTICATED_PATH,
	fetchSessionUser,
	resolvePostLoginPath
} from './session';

const sessionUser: AuthUser = {
	id: 'user-1',
	name: 'Dev User',
	email: 'dev@example.com',
	role: 'dev',
	status: 'active'
};

describe('buildLoginRedirectPath', () => {
	it('omits redirectTo for the default dashboard path', () => {
		const url = new URL('https://example.com/dashboard');

		expect(buildLoginRedirectPath(url)).toBe('/login');
	});

	it('preserves the requested dashboard path for deep links', () => {
		const url = new URL('https://example.com/dashboard/users?page=2');

		expect(buildLoginRedirectPath(url)).toBe('/login?redirectTo=%2Fdashboard%2Fusers%3Fpage%3D2');
	});
});

describe('resolvePostLoginPath', () => {
	it('uses a valid dashboard redirect target', () => {
		const url = new URL('https://example.com/login?redirectTo=%2Fdashboard%2Fstores');

		expect(resolvePostLoginPath(url)).toBe('/dashboard/stores');
	});

	it('falls back to the default dashboard path for unsafe redirects', () => {
		const url = new URL('https://example.com/login?redirectTo=https://evil.example.com');

		expect(resolvePostLoginPath(url)).toBe(DEFAULT_AUTHENTICATED_PATH);
	});
});

describe('fetchSessionUser', () => {
	it('returns the session user when auth lookup succeeds', async () => {
		const fetchMock = vi.fn(async () => {
			return new Response(JSON.stringify(sessionUser), {
				status: 200,
				headers: { 'content-type': 'application/json' }
			});
		});

		const result = await fetchSessionUser({
			fetch: fetchMock as typeof fetch,
			request: 'http://127.0.0.1:8080/api/v1/auth/me',
			cookieHeader: 'session_id=abc123'
		});

		expect(result).toEqual(sessionUser);

		const [, options] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
		const headers = new Headers(options.headers);

		expect(options.method).toBe('GET');
		expect(options.credentials).toBe('include');
		expect(headers.get('cookie')).toBe('session_id=abc123');
		expect(headers.get('accept')).toBe('application/json');
	});

	it('returns null for unauthorized responses', async () => {
		const fetchMock = vi.fn(async () => new Response(null, { status: 401 }));

		const result = await fetchSessionUser({
			fetch: fetchMock as typeof fetch,
			request: 'http://127.0.0.1:8080/api/v1/auth/me'
		});

		expect(result).toBeNull();
	});

	it('throws for unexpected backend failures', async () => {
		const fetchMock = vi.fn(async () => new Response(null, { status: 500 }));

		await expect(
			fetchSessionUser({
				fetch: fetchMock as typeof fetch,
				request: 'http://127.0.0.1:8080/api/v1/auth/me'
			})
		).rejects.toThrow('Failed to resolve session (500)');
	});
});
