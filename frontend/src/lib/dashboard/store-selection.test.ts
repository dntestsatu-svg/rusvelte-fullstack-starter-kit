import { describe, expect, it } from 'vitest';

import { deriveStoreSelection, type StoreOption } from './store-selection';

describe('deriveStoreSelection', () => {
	const stores: StoreOption[] = [
		{ id: 'store-1', name: 'Alpha Store' },
		{ id: 'store-2', name: 'Beta Store' }
	];

	it('requires explicit selection when multiple stores and no selected id', () => {
		const result = deriveStoreSelection(stores, '');

		expect(result.requiresSelection).toBe(true);
		expect(result.selectedStoreId).toBe('');
		expect(result.hasMultiple).toBe(true);
	});

	it('resolves selected store when id matches', () => {
		const result = deriveStoreSelection(stores, 'store-2');

		expect(result.requiresSelection).toBe(false);
		expect(result.selectedStoreId).toBe('store-2');
		expect(result.selectedStoreName).toBe('Beta Store');
		expect(result.hasMultiple).toBe(true);
	});

	it('auto-selects when only one store is available', () => {
		const singleStore = [{ id: 'store-1', name: 'Alpha Store' }];
		const result = deriveStoreSelection(singleStore, '');

		expect(result.requiresSelection).toBe(false);
		expect(result.selectedStoreId).toBe('store-1');
		expect(result.selectedStoreName).toBe('Alpha Store');
		expect(result.hasMultiple).toBe(false);
	});

	it('returns empty selection when no stores', () => {
		const result = deriveStoreSelection([], '');

		expect(result.requiresSelection).toBe(false);
		expect(result.selectedStoreId).toBe('');
		expect(result.hasMultiple).toBe(false);
	});
});
