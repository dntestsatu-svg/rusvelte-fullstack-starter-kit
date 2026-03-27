import { get } from 'svelte/store';
import { goto } from '$app/navigation';
import { authStore } from './store.js';

/**
 * Validates if the user is authenticated and has the correct permissions.
 * Redirects to /login if not authenticated.
 */
export function protectRoute(url: URL) {
	const state = get(authStore);
	
	if (!state.isAuthenticated && url.pathname.startsWith('/dashboard')) {
		void goto('/login');
	}
}
