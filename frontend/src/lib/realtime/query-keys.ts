import type { NotificationStatus } from '$lib/api/notifications';
import type { PaymentStatus } from '$lib/api/payments';

export const paymentQueryKeys = {
	all: ['payments'] as const,
	list: (params: { page: number; perPage: number; search: string; status: PaymentStatus | 'all' }) =>
		['payments', 'list', params] as const,
	detail: (paymentId: string) => ['payments', 'detail', paymentId] as const,
	distribution: () => ['payments', 'distribution'] as const
};

export const balanceQueryKeys = {
	all: ['balances'] as const,
	store: (storeId: string) => ['balances', 'store', storeId] as const
};

export const notificationQueryKeys = {
	all: ['notifications'] as const,
	list: (params: { page: number; perPage: number; status: NotificationStatus | 'all' }) =>
		['notifications', 'list', params] as const,
	bell: () => ['notifications', 'bell'] as const
};
