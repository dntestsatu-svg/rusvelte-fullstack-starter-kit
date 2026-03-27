import { describe, expect, it, vi } from 'vitest';

import { invalidateRealtimeQueries, reconnectDelay } from './events';

describe('reconnectDelay', () => {
	it('uses bounded exponential backoff', () => {
		expect(reconnectDelay(0)).toBe(1000);
		expect(reconnectDelay(1)).toBe(2000);
		expect(reconnectDelay(2)).toBe(4000);
		expect(reconnectDelay(10)).toBe(10000);
	});
});

describe('invalidateRealtimeQueries', () => {
	it('invalidates payment and notification queries for payment updates', async () => {
		const invalidateQueries = vi.fn().mockResolvedValue(undefined);

		await invalidateRealtimeQueries(
			{ invalidateQueries } as unknown as Parameters<typeof invalidateRealtimeQueries>[0],
			{ type: 'payment.updated', data: { payment_id: 'p-1' } }
		);

		expect(invalidateQueries).toHaveBeenCalledTimes(3);
		expect(invalidateQueries).toHaveBeenNthCalledWith(1, { queryKey: ['payments'] });
		expect(invalidateQueries).toHaveBeenNthCalledWith(2, { queryKey: ['balances'] });
		expect(invalidateQueries).toHaveBeenNthCalledWith(3, { queryKey: ['notifications'] });
	});

	it('invalidates only notification queries for notification events', async () => {
		const invalidateQueries = vi.fn().mockResolvedValue(undefined);

		await invalidateRealtimeQueries(
			{ invalidateQueries } as unknown as Parameters<typeof invalidateRealtimeQueries>[0],
			{ type: 'notification.created', data: { related_id: 'n-1' } }
		);

		expect(invalidateQueries).toHaveBeenCalledTimes(1);
		expect(invalidateQueries).toHaveBeenCalledWith({ queryKey: ['notifications'] });
	});

	it('invalidates balance queries for store balance updates', async () => {
		const invalidateQueries = vi.fn().mockResolvedValue(undefined);

		await invalidateRealtimeQueries(
			{ invalidateQueries } as unknown as Parameters<typeof invalidateRealtimeQueries>[0],
			{ type: 'store.balance.updated', data: { store_id: 'store-1' } }
		);

		expect(invalidateQueries).toHaveBeenCalledTimes(1);
		expect(invalidateQueries).toHaveBeenCalledWith({ queryKey: ['balances'] });
	});
});
