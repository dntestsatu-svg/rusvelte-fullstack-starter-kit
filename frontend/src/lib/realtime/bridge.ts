import type { RealtimeEventName } from './events';

export type RealtimeMessageHandler = (event: MessageEvent<string>) => void;

export interface RealtimeEventSourceLike {
	onopen: ((event: Event) => void) | null;
	onerror: ((event: Event) => void) | null;
	addEventListener(type: RealtimeEventName, listener: RealtimeMessageHandler): void;
	close(): void;
}

export interface RealtimeBridgeOptions {
	createSource: () => RealtimeEventSourceLike;
	onEvent: (type: RealtimeEventName, event: MessageEvent<string>) => Promise<void> | void;
	reconnectDelay: (attempt: number) => number;
	setTimer?: (handler: () => void, timeout: number) => ReturnType<typeof setTimeout>;
	clearTimer?: (handle: ReturnType<typeof setTimeout>) => void;
}

export function startRealtimeBridge(options: RealtimeBridgeOptions): () => void {
	const setTimer = options.setTimer ?? ((handler, timeout) => setTimeout(handler, timeout));
	const clearTimer = options.clearTimer ?? ((handle) => clearTimeout(handle));

	let disposed = false;
	let reconnectAttempts = 0;
	let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
	let source: RealtimeEventSourceLike | null = null;

	const handleRealtimeEvent = async (
		type: RealtimeEventName,
		event: MessageEvent<string>
	): Promise<void> => {
		reconnectAttempts = 0;
		await options.onEvent(type, event);
	};

	const connect = () => {
		if (disposed) {
			return;
		}

		source = options.createSource();
		source.onopen = () => {
			reconnectAttempts = 0;
		};
		source.addEventListener('payment.updated', (event) => {
			void handleRealtimeEvent('payment.updated', event);
		});
		source.addEventListener('notification.created', (event) => {
			void handleRealtimeEvent('notification.created', event);
		});
		source.onerror = () => {
			source?.close();
			if (disposed) {
				return;
			}

			const delay = options.reconnectDelay(reconnectAttempts);
			reconnectAttempts += 1;
			reconnectTimer = setTimer(connect, delay);
		};
	};

	connect();

	return () => {
		disposed = true;
		source?.close();
		if (reconnectTimer !== null) {
			clearTimer(reconnectTimer);
		}
	};
}
