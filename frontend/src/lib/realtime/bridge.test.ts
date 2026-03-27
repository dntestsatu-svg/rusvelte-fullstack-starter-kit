import { afterEach, describe, expect, it, vi } from 'vitest';

import { startRealtimeBridge, type RealtimeEventSourceLike } from './bridge';

class FakeEventSource implements RealtimeEventSourceLike {
	onopen: (() => void) | null = null;
	onerror: (() => void) | null = null;
	listeners = new Map<string, (event: MessageEvent<string>) => void>();
	close = vi.fn();

	addEventListener(type: string, listener: (event: MessageEvent<string>) => void): void {
		this.listeners.set(type, listener);
	}

	emitOpen(): void {
		this.onopen?.();
	}

	emitError(): void {
		this.onerror?.();
	}

	emit(type: string, data: string): void {
		this.listeners.get(type)?.({ data } as MessageEvent<string>);
	}
}

describe('startRealtimeBridge', () => {
	afterEach(() => {
		vi.useRealTimers();
	});

	it('reconnects after an error using the provided delay strategy', () => {
		vi.useFakeTimers();
		const sources: FakeEventSource[] = [];

		const dispose = startRealtimeBridge({
			createSource: () => {
				const source = new FakeEventSource();
				sources.push(source);
				return source;
			},
			onEvent: vi.fn(),
			reconnectDelay: (attempt) => (attempt + 1) * 1000
		});

		expect(sources).toHaveLength(1);
		sources[0].emitError();

		expect(sources[0].close).toHaveBeenCalledTimes(1);
		expect(sources).toHaveLength(1);

		vi.advanceTimersByTime(999);
		expect(sources).toHaveLength(1);

		vi.advanceTimersByTime(1);
		expect(sources).toHaveLength(2);

		dispose();
	});

	it('forwards payment and notification events to the consumer', async () => {
		const source = new FakeEventSource();
		const onEvent = vi.fn().mockResolvedValue(undefined);

		const dispose = startRealtimeBridge({
			createSource: () => source,
			onEvent,
			reconnectDelay: () => 1000
		});

		source.emit('payment.updated', '{"payment_id":"payment-1"}');
		source.emit('notification.created', '{"related_id":"notification-1"}');
		source.emit('store.balance.updated', '{"store_id":"store-1"}');

		await Promise.resolve();

		expect(onEvent).toHaveBeenCalledTimes(3);
		expect(onEvent).toHaveBeenNthCalledWith(
			1,
			'payment.updated',
			expect.objectContaining({ data: '{"payment_id":"payment-1"}' })
		);
		expect(onEvent).toHaveBeenNthCalledWith(
			2,
			'notification.created',
			expect.objectContaining({ data: '{"related_id":"notification-1"}' })
		);
		expect(onEvent).toHaveBeenNthCalledWith(
			3,
			'store.balance.updated',
			expect.objectContaining({ data: '{"store_id":"store-1"}' })
		);

		dispose();
	});

	it('stops reconnect scheduling after dispose', () => {
		vi.useFakeTimers();
		const sources: FakeEventSource[] = [];

		const dispose = startRealtimeBridge({
			createSource: () => {
				const source = new FakeEventSource();
				sources.push(source);
				return source;
			},
			onEvent: vi.fn(),
			reconnectDelay: () => 1000
		});

		sources[0].emitError();
		dispose();
		vi.advanceTimersByTime(1000);

		expect(sources).toHaveLength(1);
		expect(sources[0].close).toHaveBeenCalledTimes(2);
	});
});
