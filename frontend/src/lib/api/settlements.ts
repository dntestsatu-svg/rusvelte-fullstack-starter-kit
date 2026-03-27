import { client } from './client';
import type { StoreBalanceSnapshot } from './stores';

export interface ProcessedSettlement {
	id: string;
	store_id: string;
	amount: number;
	status: 'processed';
	processed_by_user_id: string;
	notes?: string | null;
	created_at: string;
}

export interface SettlementProcessResponse {
	settlement: ProcessedSettlement;
	balance: StoreBalanceSnapshot;
}

export const settlementsApi = {
	create: async (data: {
		store_id: string;
		amount: number;
		notes?: string;
	}) => {
		return client.post<SettlementProcessResponse>('/api/v1/dev/settlements', data);
	}
};
