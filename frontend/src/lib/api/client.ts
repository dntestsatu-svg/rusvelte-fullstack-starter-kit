/**
 * Minimal API client for JustQiu Issue #5.
 * Handles credential inclusion and CSRF token injection.
 */
export async function apiFetch(url: string, options: RequestInit = {}) {
	const headers = new Headers(options.headers);
	
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

	if (!response.ok) {
		const error = await response.json().catch(() => ({ message: 'An unexpected error occurred' }));
		throw new Error(error.message || 'API request failed');
	}

	return response.json();
}

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
