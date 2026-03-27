import type { QueryClient } from '@tanstack/svelte-query';

import { notificationQueryKeys, paymentQueryKeys } from './query-keys';

export type RealtimeEventName = 'payment.updated' | 'notification.created';

export interface RealtimeEventPayload {
	type: RealtimeEventName;
	data: unknown;
}

export function reconnectDelay(attempt: number): number {
	return Math.min(1000 * 2 ** attempt, 10000);
}

export async function invalidateRealtimeQueries(
	queryClient: QueryClient,
	event: RealtimeEventPayload
): Promise<void> {
	if (event.type === 'payment.updated') {
		await queryClient.invalidateQueries({ queryKey: paymentQueryKeys.all });
		await queryClient.invalidateQueries({ queryKey: notificationQueryKeys.all });
		return;
	}

	if (event.type === 'notification.created') {
		await queryClient.invalidateQueries({ queryKey: notificationQueryKeys.all });
	}
}
