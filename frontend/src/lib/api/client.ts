/**
 * Minimal API client for JustQiu Issue #5.
 * Handles credential inclusion and CSRF token injection.
 */
export class ApiError extends Error {
	status: number;

	constructor(message: string, status: number) {
		super(message);
		this.name = 'ApiError';
		this.status = status;
	}
}

export async function apiFetch<T = unknown>(url: string, options: RequestInit = {}): Promise<T> {
	const headers = new Headers(options.headers);

	if (options.body && !headers.has('Content-Type')) {
		headers.set('Content-Type', 'application/json');
	}
	
	// Inject CSRF token from a cookie or meta tag if available
	// For now, we assume it's available in a known COOKIE or provided globally
	const csrfToken = getCsrfToken();
	if (csrfToken) {
		headers.set('X-CSRF-Token', csrfToken);
	}

	const response = await fetch(url, {
		...options,
		headers,
		credentials: 'include' // Required for session-based auth
	});

	const rawBody = await response.text();
	const contentType = response.headers.get('content-type') ?? '';
	const parsedBody = rawBody
		? contentType.includes('application/json')
			? JSON.parse(rawBody)
			: rawBody
		: undefined;

	if (!response.ok) {
		const message =
			typeof parsedBody === 'object' && parsedBody !== null
				? ((parsedBody as { error?: { message?: string }; message?: string }).error?.message ??
					(parsedBody as { message?: string }).message ??
					'API request failed')
				: (parsedBody as string | undefined) ?? 'API request failed';
		throw new ApiError(message, response.status);
	}

	return parsedBody as T;
}

/**
 * Client object for easier imports in components
 */
export const client = {
	get: <T>(url: string) => apiFetch<T>(url, { method: 'GET' }),
	post: <T>(url: string, body?: unknown) =>
		apiFetch<T>(url, { method: 'POST', body: body === undefined ? undefined : JSON.stringify(body) }),
	put: <T>(url: string, body: unknown) => apiFetch<T>(url, { method: 'PUT', body: JSON.stringify(body) }),
	delete: <T>(url: string) => apiFetch<T>(url, { method: 'DELETE' }),
	patch: <T>(url: string, body: unknown) => apiFetch<T>(url, { method: 'PATCH', body: JSON.stringify(body) }),
};

function getCsrfToken(): string | null {
	if (typeof document === 'undefined') return null;
	
	// Typical pattern: read from Cookie
	const name = "XSRF-TOKEN=";
	const decodedCookie = decodeURIComponent(document.cookie);
	const ca = decodedCookie.split(';');
	for (let i = 0; i < ca.length; i++) {
		let c = ca[i];
		while (c.charAt(0) === ' ') {
			c = c.substring(1);
		}
		if (c.indexOf(name) === 0) {
			return c.substring(name.length, c.length);
		}
	}
	return null;
}
