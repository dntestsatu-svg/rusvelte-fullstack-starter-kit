import { writable } from 'svelte/store';

export interface User {
	id: string;
	email: string;
	role: 'admin' | 'merchant' | 'user';
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
		isLoading: true
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
