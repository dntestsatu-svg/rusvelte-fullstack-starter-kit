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

describe('storesApi bank endpoints', () => {
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

	it('posts inquiry and create bank requests to the scoped store endpoints with CSRF', async () => {
		const fetchMock = vi
			.fn()
			.mockResolvedValueOnce(
				new Response(
					JSON.stringify({
						inquiry: {
							bank_code: '014',
							bank_name: 'PT. BANK CENTRAL ASIA TBK',
							account_holder_name: 'Alice Owner',
							account_number_last4: '7890',
							provider_fee_amount: 1800,
							partner_ref_no: 'partner-ref-15',
							vendor_ref_no: 'vendor-ref-15',
							inquiry_id: 1550
						}
					}),
					{ status: 200, headers: { 'content-type': 'application/json' } }
				)
			)
			.mockResolvedValueOnce(
				new Response(
					JSON.stringify({
						bank: {
							id: 'bank-1',
							store_id: 'store-1',
							owner_user_id: 'user-1',
							bank_code: '014',
							bank_name: 'PT. BANK CENTRAL ASIA TBK',
							account_holder_name: 'Alice Owner',
							account_number_last4: '7890',
							is_default: true,
							verification_status: 'verified',
							verified_at: '2026-03-27T00:00:00Z',
							created_at: '2026-03-27T00:00:00Z',
							updated_at: '2026-03-27T00:00:00Z'
						}
					}),
					{ status: 201, headers: { 'content-type': 'application/json' } }
				)
			);
		vi.stubGlobal('fetch', fetchMock);

		await storesApi.inquireBank('store-1', {
			bank_code: '014',
			account_number: '1234567890'
		});
		await storesApi.createBank('store-1', {
			bank_code: '014',
			account_number: '1234567890',
			is_default: true
		});

		const [inquiryUrl, inquiryOptions] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
		const [createUrl, createOptions] = fetchMock.mock.calls[1] as unknown as [string, RequestInit];

		expect(inquiryUrl).toBe('/api/v1/stores/store-1/banks/inquiry');
		expect(createUrl).toBe('/api/v1/stores/store-1/banks');
		expect(new Headers(inquiryOptions.headers).get('X-CSRF-Token')).toBe('csrf-123');
		expect(new Headers(createOptions.headers).get('X-CSRF-Token')).toBe('csrf-123');
		expect(createOptions.body).toBe(
			JSON.stringify({
				bank_code: '014',
				account_number: '1234567890',
				is_default: true
			})
		);
	});

	it('posts set default requests to the scoped bank endpoint with CSRF', async () => {
		const fetchMock = vi.fn(async () => {
			return new Response(
				JSON.stringify({
					bank: {
						id: 'bank-1',
						store_id: 'store-1',
						owner_user_id: 'user-1',
						bank_code: '014',
						bank_name: 'PT. BANK CENTRAL ASIA TBK',
						account_holder_name: 'Alice Owner',
						account_number_last4: '7890',
						is_default: true,
						verification_status: 'verified',
						verified_at: '2026-03-27T00:00:00Z',
						created_at: '2026-03-27T00:00:00Z',
						updated_at: '2026-03-27T00:00:00Z'
					}
				}),
				{ status: 200, headers: { 'content-type': 'application/json' } }
			);
		});
		vi.stubGlobal('fetch', fetchMock);

		await storesApi.setDefaultBank('store-1', 'bank-1');

		const [url, options] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
		expect(url).toBe('/api/v1/stores/store-1/banks/bank-1/default');
		expect(options.method).toBe('POST');
		expect(new Headers(options.headers).get('X-CSRF-Token')).toBe('csrf-123');
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
