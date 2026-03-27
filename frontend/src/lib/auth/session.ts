import type { AuthUser } from '../api/auth';

export const SESSION_DEPENDENCY = 'app:auth-session';
export const DEFAULT_AUTHENTICATED_PATH = '/dashboard';
const LOGIN_PATH = '/login';
const DASHBOARD_PATH_PREFIX = '/dashboard';

function isDashboardPath(path: string): boolean {
	return path === DASHBOARD_PATH_PREFIX || path.startsWith(`${DASHBOARD_PATH_PREFIX}/`);
}

function normalizeInternalPath(path: string | null): string | null {
	if (!path || !path.startsWith('/') || path.startsWith('//')) {
		return null;
	}

	return path;
}

export function buildLoginRedirectPath(url: URL): string {
	const redirectTo = `${url.pathname}${url.search}`;
	const params = new URLSearchParams();

	if (redirectTo !== DEFAULT_AUTHENTICATED_PATH) {
		params.set('redirectTo', redirectTo);
	}

	const query = params.toString();
	return query ? `${LOGIN_PATH}?${query}` : LOGIN_PATH;
}

export function resolvePostLoginPath(url: URL): string {
	const redirectTo = normalizeInternalPath(url.searchParams.get('redirectTo'));

	if (!redirectTo || !isDashboardPath(redirectTo)) {
		return DEFAULT_AUTHENTICATED_PATH;
	}

	return redirectTo;
}

export async function fetchSessionUser({
	fetch,
	request,
	cookieHeader
}: {
	fetch: typeof globalThis.fetch;
	request: string | URL;
	cookieHeader?: string | null;
}): Promise<AuthUser | null> {
	const headers = new Headers({ accept: 'application/json' });

	if (cookieHeader) {
		headers.set('cookie', cookieHeader);
	}

	const response = await fetch(request, {
		method: 'GET',
		headers,
		credentials: 'include'
	});

	if (response.status === 401 || response.status === 403) {
		return null;
	}

	if (!response.ok) {
		throw new Error(`Failed to resolve session (${response.status})`);
	}

	return (await response.json()) as AuthUser;
}
