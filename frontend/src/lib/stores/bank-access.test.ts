import { describe, expect, it } from 'vitest';

import {
	applyDefaultBankRecord,
	canManageStoreBanks,
	canViewStoreBanks,
	formatMaskedBankAccount,
	mergeSavedBankRecord,
	shouldDefaultNewBank
} from './bank-access';

describe('bank access helpers', () => {
	it('allows only dev, superadmin, and store owner to view bank accounts', () => {
		expect(
			canViewStoreBanks({
				storeOwnerUserId: 'owner-1',
				sessionUserId: 'owner-1',
				sessionUserRole: 'user'
			})
		).toBe(true);
		expect(
			canViewStoreBanks({
				storeOwnerUserId: 'owner-1',
				sessionUserId: 'dev-1',
				sessionUserRole: 'dev'
			})
		).toBe(true);
		expect(
			canViewStoreBanks({
				storeOwnerUserId: 'owner-1',
				sessionUserId: 'super-1',
				sessionUserRole: 'superadmin'
			})
		).toBe(true);
		expect(
			canViewStoreBanks({
				storeOwnerUserId: 'owner-1',
				sessionUserId: 'manager-1',
				sessionUserRole: 'admin'
			})
		).toBe(false);
	});

	it('allows only dev and store owner to manage bank accounts', () => {
		expect(
			canManageStoreBanks({
				storeOwnerUserId: 'owner-1',
				sessionUserId: 'owner-1',
				sessionUserRole: 'user'
			})
		).toBe(true);
		expect(
			canManageStoreBanks({
				storeOwnerUserId: 'owner-1',
				sessionUserId: 'dev-1',
				sessionUserRole: 'dev'
			})
		).toBe(true);
		expect(
			canManageStoreBanks({
				storeOwnerUserId: 'owner-1',
				sessionUserId: 'super-1',
				sessionUserRole: 'superadmin'
			})
		).toBe(false);
	});

	it('formats masked bank numbers from last4 only', () => {
		expect(formatMaskedBankAccount('7890')).toBe('**** 7890');
	});

	it('reconciles a newly saved default bank by clearing older defaults in local state', () => {
		const merged = mergeSavedBankRecord(
			[
				{ id: 'bank-1', is_default: true },
				{ id: 'bank-2', is_default: false }
			],
			{ id: 'bank-3', is_default: true }
		);

		expect(merged).toEqual([
			{ id: 'bank-3', is_default: true },
			{ id: 'bank-1', is_default: false },
			{ id: 'bank-2', is_default: false }
		]);
	});

	it('applies a new default bank selection across existing records', () => {
		const updated = applyDefaultBankRecord(
			[
				{ id: 'bank-1', is_default: true },
				{ id: 'bank-2', is_default: false }
			],
			'bank-2'
		);

		expect(updated).toEqual([
			{ id: 'bank-1', is_default: false },
			{ id: 'bank-2', is_default: true }
		]);
	});

	it('defaults the first bank only when the store has no saved bank yet', () => {
		expect(shouldDefaultNewBank(0)).toBe(true);
		expect(shouldDefaultNewBank(1)).toBe(false);
	});
});
