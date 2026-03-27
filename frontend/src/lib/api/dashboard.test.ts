import { describe, expect, it, vi } from 'vitest';

import { dashboardApi } from './dashboard';

describe('dashboardApi provider balance', () => {
	it('requests provider balance for dev dashboard cards', async () => {
		const fetchMock = vi.fn(async () => {
			return new Response(
				JSON.stringify({ provider_pending_balance: 100, provider_settle_balance: 200 }),
				{
					status: 200,
					headers: { 'content-type': 'application/json' }
				}
			);
		});
		vi.stubGlobal('fetch', fetchMock);

		await dashboardApi.getProviderBalance();

		const [url, options] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
		expect(url).toBe('/api/v1/dev/provider/balance');
		expect(options.method).toBe('GET');
		expect(options.credentials).toBe('include');
	});
});
