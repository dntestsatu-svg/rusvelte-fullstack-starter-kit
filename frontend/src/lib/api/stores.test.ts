import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ApiError, apiFetch } from './client';
import { storesApi } from './stores';

describe('storesApi token endpoints', () => {
	const originalDocument = globalThis.document;

	beforeEach(() => {
		Object.defineProperty(globalThis, 'document', {
			value: { cookie: 'XSRF-TOKEN=csrf-123' },
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

	it('posts create token requests to the scoped store endpoint with CSRF', async () => {
		const fetchMock = vi.fn(async () => {
			return new Response(
				JSON.stringify({
					token: {
						id: 'token-1',
						name: 'Primary Token',
						display_prefix: 'jq_sk_****abcd',
						last_used_at: null,
						expires_at: null,
						created_at: '2026-03-27T00:00:00Z'
					},
					plaintext_token: 'jq_sk_abcd1234efgh.secret'
				}),
				{
					status: 201,
					headers: { 'content-type': 'application/json' }
				}
			);
		});
		vi.stubGlobal('fetch', fetchMock);

		await storesApi.createToken('store-1', { name: 'Primary Token' });

		const [url, options] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
		const headers = new Headers(options.headers);

		expect(url).toBe('/api/v1/stores/store-1/tokens');
		expect(options.method).toBe('POST');
		expect(options.credentials).toBe('include');
		expect(headers.get('X-CSRF-Token')).toBe('csrf-123');
		expect(options.body).toBe(JSON.stringify({ name: 'Primary Token' }));
	});

	it('sends revoke requests to the scoped token endpoint with CSRF', async () => {
		const fetchMock = vi.fn(async () => new Response(null, { status: 204 }));
		vi.stubGlobal('fetch', fetchMock);

		await storesApi.revokeToken('store-1', 'token-1');

		const [url, options] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
		const headers = new Headers(options.headers);

		expect(url).toBe('/api/v1/stores/store-1/tokens/token-1');
		expect(options.method).toBe('DELETE');
		expect(options.credentials).toBe('include');
		expect(headers.get('X-CSRF-Token')).toBe('csrf-123');
	});
});

describe('storesApi balance endpoints', () => {
	it('requests store balance snapshot using scoped store id', async () => {
		const fetchMock = vi.fn(async () => {
			return new Response(
				JSON.stringify({
					balance: {
						store_id: 'store-1',
						pending_balance: 1200,
						settled_balance: 400,
						reserved_settled_balance: 100,
						withdrawable_balance: 300,
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

		await storesApi.getBalances('store-1');

		const [url, options] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
		expect(url).toBe('/api/v1/stores/store-1/balances');
		expect(options.method).toBe('GET');
		expect(options.credentials).toBe('include');
	});
});

describe('apiFetch', () => {
	it('throws ApiError with status metadata for forbidden responses', async () => {
		const fetchMock = vi.fn(async () => {
			return new Response(
				JSON.stringify({
					error: { message: 'You do not have permission to perform this action' }
				}),
				{
					status: 403,
					headers: { 'content-type': 'application/json' }
				}
			);
		});
		vi.stubGlobal('fetch', fetchMock);

		await expect(apiFetch('/api/v1/stores/store-1/tokens')).rejects.toMatchObject({
			name: 'ApiError',
			status: 403,
			message: 'You do not have permission to perform this action'
		});
	});
});
