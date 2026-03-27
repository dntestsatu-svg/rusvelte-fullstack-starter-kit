import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';

import SettlementCenterView from './SettlementCenterView.svelte';

describe('SettlementCenterView', () => {
	it('renders unauthorized state for non-dev users', () => {
		const { html } = render(SettlementCenterView, {
			props: {
				isDev: false
			}
		});

		expect(html).toContain('Access restricted');
		expect(html).toContain('developer accounts');
	});

	it('renders explicit multi-store selector and settlement copy', () => {
		const { html } = render(SettlementCenterView, {
			props: {
				isDev: true,
				storeState: 'ready',
				storeOptions: [
					{ id: 'store-1', name: 'Alpha Store' },
					{ id: 'store-2', name: 'Beta Store' }
				],
				selectedStoreId: '',
				activeStoreName: '',
				requiresSelection: true,
				balanceState: 'idle',
				amountValue: '',
				notesValue: '',
				submitState: 'idle'
			}
		});

		expect(html).toContain('Settlement Rule');
		expect(html).toContain('It does not create new funds');
		expect(html).toContain('Select a store before entering a settlement amount');
		expect(html).toContain('Select a store');
	});

	it('renders active store label and success summary', () => {
		const { html } = render(SettlementCenterView, {
			props: {
				isDev: true,
				storeState: 'ready',
				storeOptions: [{ id: 'store-1', name: 'Alpha Store' }],
				selectedStoreId: 'store-1',
				activeStoreName: 'Alpha Store',
				requiresSelection: false,
				balanceState: 'ready',
				balance: {
					pending_balance: 90000,
					settled_balance: 75000,
					withdrawable_balance: 70000
				},
				amountValue: '30000',
				notesValue: 'Manual note',
				submitState: 'success',
				receipt: {
					id: 'settlement-1',
					storeName: 'Alpha Store',
					amount: 30000,
					pending_balance: 90000,
					settled_balance: 75000,
					withdrawable_balance: 70000,
					created_at: '2026-03-27T00:00:00Z'
				}
			}
		});

		expect(html).toContain('Active store: Alpha Store');
		expect(html).toContain('Pending Balance');
		expect(html).toContain('Settled Balance');
		expect(html).toContain('Withdrawable Balance');
		expect(html).toContain('Settlement processed');
	});
});
