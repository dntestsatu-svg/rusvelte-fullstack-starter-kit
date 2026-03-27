import { writable } from 'svelte/store';

export interface User {
	id: string;
	name: string;
	email: string;
	role: 'admin' | 'user' | 'dev' | 'superadmin';
	status: 'active' | 'inactive' | 'suspended';
}

export interface AuthState {
	user: User | null;
	isAuthenticated: boolean;
	isLoading: boolean;
}

function createAuthStore() {
	const { subscribe, set, update } = writable<AuthState>({
		user: null,
		isAuthenticated: false,
		isLoading: false
	});

	return {
		subscribe,
		setAuth: (user: User | null) => {
			set({
				user,
				isAuthenticated: !!user,
				isLoading: false
			});
		},
		setLoading: (isLoading: boolean) => {
			update((s) => ({ ...s, isLoading }));
		},
		reset: () => {
			set({
				user: null,
				isAuthenticated: false,
				isLoading: false
			});
		}
	};
}

export const authStore = createAuthStore();

import { derived } from 'svelte/store';
export const user = derived(authStore, $s => $s.user);
