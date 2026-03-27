import { describe, expect, it, vi } from 'vitest';

import { paymentsApi } from './payments';

describe('paymentsApi distribution endpoint', () => {
	it('requests payment distribution data', async () => {
		const fetchMock = vi.fn(async () => {
			return new Response(
				JSON.stringify({
					distribution: { success: 2, failed: 1, expired: 0 }
				}),
				{
					status: 200,
					headers: { 'content-type': 'application/json' }
				}
			);
		});
		vi.stubGlobal('fetch', fetchMock);

		await paymentsApi.getDistribution();

		const [url, options] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
		expect(url).toBe('/api/v1/payments/distribution');
		expect(options.method).toBe('GET');
		expect(options.credentials).toBe('include');
	});
});
