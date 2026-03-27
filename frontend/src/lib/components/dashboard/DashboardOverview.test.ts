import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';

import DashboardOverview from './DashboardOverview.svelte';

describe('DashboardOverview', () => {
	it('renders balance loading state', () => {
		const { html } = render(DashboardOverview, {
			props: {
				balanceState: 'loading',
				distributionState: 'loading'
			}
		});

		expect(html).toContain('Balance Overview');
		expect(html).toContain('Payment Distribution');
	});

	it('renders balance error state message', () => {
		const { html } = render(DashboardOverview, {
			props: {
				balanceState: 'error',
				balanceErrorMessage: 'Balance failed',
				distributionState: 'ready',
				distribution: { success: 1, failed: 0, expired: 0 }
			}
		});

		expect(html).toContain('Balance summary unavailable');
		expect(html).toContain('Balance failed');
	});

	it('renders balance empty state message', () => {
		const { html } = render(DashboardOverview, {
			props: {
				balanceState: 'empty',
				balanceEmptyMessage: 'No store yet',
				distributionState: 'ready',
				distribution: { success: 0, failed: 0, expired: 0 }
			}
		});

		expect(html).toContain('No balances yet');
		expect(html).toContain('No store yet');
	});

	it('renders distribution empty state message', () => {
		const { html } = render(DashboardOverview, {
			props: {
				balanceState: 'ready',
				balanceCards: [
					{
						label: 'Pending Balance',
						value: 'Rp 0',
						description: 'Pending',
						tooltip: 'Pending tooltip'
					}
				],
				distributionState: 'empty',
				distributionEmptyMessage: 'No data yet'
			}
		});

		expect(html).toContain('No finalized payments');
		expect(html).toContain('No data yet');
	});

	it('renders balance tooltips for all cards', () => {
		const { html } = render(DashboardOverview, {
			props: {
				balanceState: 'ready',
				balanceCards: [
					{
						label: 'Pending Balance',
						value: 'Rp 120.000',
						description: 'Pending',
						tooltip: 'Pending tooltip text'
					},
					{
						label: 'Settled Balance',
						value: 'Rp 45.000',
						description: 'Settled',
						tooltip: 'Settled tooltip text'
					},
					{
						label: 'Withdrawable Balance',
						value: 'Rp 40.000',
						description: 'Withdrawable',
						tooltip: 'Withdrawable tooltip text'
					}
				],
				distributionState: 'ready',
				distribution: { success: 1, failed: 1, expired: 1 }
			}
		});

		expect(html).toContain('Pending tooltip text');
		expect(html).toContain('Settled tooltip text');
		expect(html).toContain('Withdrawable tooltip text');
	});
});
