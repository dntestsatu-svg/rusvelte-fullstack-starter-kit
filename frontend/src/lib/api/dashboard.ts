import { client } from './client';

export interface ProviderBalanceSnapshot {
	provider_pending_balance: number;
	provider_settle_balance: number;
}

export const dashboardApi = {
	getProviderBalance: async () => {
		return client.get<ProviderBalanceSnapshot>('/api/v1/dev/provider/balance');
	}
};
