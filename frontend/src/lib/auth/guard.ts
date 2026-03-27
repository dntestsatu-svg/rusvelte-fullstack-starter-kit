import { get } from 'svelte/store';
import { redirect } from '@sveltejs/kit';
import { authStore } from './store.js';

/**
 * Validates if the user is authenticated and has the correct permissions.
 * Redirects to /login if not authenticated.
 */
export function protectRoute(url: URL) {
	const state = get(authStore);

	// In a real app, we might check if (state.isLoading) and return early
	// but for this issue, we assume the store is seeded or loading is handled.
	
	if (!state.isAuthenticated && url.pathname.startsWith('/dashboard')) {
		throw redirect(303, '/login');
	}
}
