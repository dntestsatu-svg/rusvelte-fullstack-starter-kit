import { describe, expect, it } from 'vitest';

import { canAccessStoreTokens, formatStoreTokenTimestamp } from './token-access';

describe('canAccessStoreTokens', () => {
	it('allows dev users regardless of store ownership', () => {
		expect(
			canAccessStoreTokens({
				storeOwnerUserId: 'store-owner',
				sessionUserId: 'someone-else',
				sessionUserRole: 'dev'
			})
		).toBe(true);
	});

	it('allows the store owner', () => {
		expect(
			canAccessStoreTokens({
				storeOwnerUserId: 'store-owner',
				sessionUserId: 'store-owner',
				sessionUserRole: 'user'
			})
		).toBe(true);
	});

	it('keeps admin and non-owners read-only in the UI mirror', () => {
		expect(
			canAccessStoreTokens({
				storeOwnerUserId: 'store-owner',
				sessionUserId: 'admin-user',
				sessionUserRole: 'admin'
			})
		).toBe(false);
		expect(
			canAccessStoreTokens({
				storeOwnerUserId: 'store-owner',
				sessionUserId: 'viewer-user',
				sessionUserRole: 'user'
			})
		).toBe(false);
	});
});

describe('formatStoreTokenTimestamp', () => {
	it('returns Never for empty values', () => {
		expect(formatStoreTokenTimestamp()).toBe('Never');
		expect(formatStoreTokenTimestamp(null)).toBe('Never');
	});
});
