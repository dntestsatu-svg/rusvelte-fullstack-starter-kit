import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { settlementsApi } from './settlements';

describe('settlementsApi', () => {
	const originalDocument = globalThis.document;

	beforeEach(() => {
		Object.defineProperty(globalThis, 'document', {
			value: { cookie: 'XSRF-TOKEN=csrf-789' },
			configurable: true
		});
	});

	afterEach(() => {
		vi.restoreAllMocks();
		if (originalDocument) {
			Object.defineProperty(globalThis, 'document', {
				value: originalDocument,
				configurable: true
			});
		} else {
			Reflect.deleteProperty(globalThis, 'document');
		}
	});

	it('posts settlement requests to the dev settlement endpoint with CSRF', async () => {
		const fetchMock = vi.fn(async () => {
			return new Response(
				JSON.stringify({
					settlement: {
						id: 'settlement-1',
						store_id: 'store-1',
						amount: 30000,
						status: 'processed',
						processed_by_user_id: 'dev-1',
						notes: 'Manual settlement',
						created_at: '2026-03-27T00:00:00Z'
					},
					balance: {
						store_id: 'store-1',
						pending_balance: 90000,
						settled_balance: 75000,
						reserved_settled_balance: 5000,
						withdrawable_balance: 70000,
						updated_at: '2026-03-27T00:00:00Z'
					}
				}),
				{
					status: 200,
					headers: { 'content-type': 'application/json' }
				}
			);
		});
		vi.stubGlobal('fetch', fetchMock);

		await settlementsApi.create({
			store_id: 'store-1',
			amount: 30000,
			notes: 'Manual settlement'
		});

		const [url, options] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
		const headers = new Headers(options.headers);

		expect(url).toBe('/api/v1/dev/settlements');
		expect(options.method).toBe('POST');
		expect(options.credentials).toBe('include');
		expect(headers.get('X-CSRF-Token')).toBe('csrf-789');
		expect(options.body).toBe(
			JSON.stringify({
				store_id: 'store-1',
				amount: 30000,
				notes: 'Manual settlement'
			})
		);
	});
});
