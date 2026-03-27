import { client } from './client';

export type PaymentStatus = 'created' | 'pending' | 'success' | 'failed' | 'expired';

export interface DashboardPaymentSummary {
	id: string;
	store_id: string;
	store_name: string;
	store_slug: string;
	gross_amount: number;
	platform_tx_fee_amount: number;
	store_pending_credit_amount: number;
	status: PaymentStatus;
	provider_trx_id?: string | null;
	merchant_order_id?: string | null;
	custom_ref?: string | null;
	expired_at?: string | null;
	finalized_at?: string | null;
	created_at: string;
	updated_at: string;
}

export interface DashboardPaymentDetail extends DashboardPaymentSummary {
	provider_name: string;
	provider_terminal_id?: string | null;
	provider_rrn?: string | null;
	platform_tx_fee_bps: number;
	qris_payload?: string | null;
	provider_created_at?: string | null;
	provider_finished_at?: string | null;
}

export interface DashboardPaymentListResponse {
	payments: DashboardPaymentSummary[];
	total: number;
	page: number;
	per_page: number;
}

export interface DashboardPaymentDistribution {
	success: number;
	failed: number;
	expired: number;
}

export const paymentsApi = {
	list: async (params: {
		page?: number;
		perPage?: number;
		search?: string;
		status?: PaymentStatus;
	}) => {
		const query = new URLSearchParams();
		if (params.page) query.append('page', params.page.toString());
		if (params.perPage) query.append('per_page', params.perPage.toString());
		if (params.search) query.append('search', params.search);
		if (params.status) query.append('status', params.status);

		return client.get<DashboardPaymentListResponse>(`/api/v1/payments?${query.toString()}`);
	},

	get: async (paymentId: string) => {
		return client.get<{ payment: DashboardPaymentDetail }>(`/api/v1/payments/${paymentId}`);
	},

	getDistribution: async () => {
		return client.get<{ distribution: DashboardPaymentDistribution }>(
			'/api/v1/payments/distribution'
		);
	}
};
