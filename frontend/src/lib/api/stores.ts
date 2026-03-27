import { client } from './client';

export interface Store {
    id: string;
    owner_user_id: string;
    owner_name: string;
    owner_email: string;
    name: string;
    slug: string;
    status: 'active' | 'inactive';
    callback_url?: string | null;
    provider_username: string;
    created_at: string;
    updated_at: string;
}

export interface StoreMember {
    id: string;
    store_id: string;
    user_id: string;
    user_name: string;
    user_email: string;
    user_platform_role: 'dev' | 'superadmin' | 'admin' | 'user';
    store_role: 'owner' | 'manager' | 'staff' | 'viewer';
    status: 'active' | 'inactive';
    invited_by?: string | null;
    created_at: string;
    updated_at: string;
}

export interface StoreApiToken {
    id: string;
    name: string;
    display_prefix: string;
    last_used_at?: string | null;
    expires_at?: string | null;
    created_at: string;
}

export interface StoreBankAccount {
    id: string;
    store_id: string;
    owner_user_id: string;
    bank_code: string;
    bank_name: string;
    account_holder_name: string;
    account_number_last4: string;
    is_default: boolean;
    verification_status: 'verified';
    verified_at?: string | null;
    created_at: string;
    updated_at: string;
}

export interface StoreBankInquiry {
    bank_code: string;
    bank_name: string;
    account_holder_name: string;
    account_number_last4: string;
    provider_fee_amount: number;
    partner_ref_no: string;
    vendor_ref_no?: string | null;
    inquiry_id: number;
}

export interface StoreBalanceSnapshot {
    store_id: string;
    pending_balance: number;
    settled_balance: number;
    reserved_settled_balance: number;
    withdrawable_balance: number;
    updated_at: string;
}

export interface PayoutPreview {
    bank_account_id: string;
    bank_name: string;
    account_holder_name: string;
    account_number_last4: string;
    requested_amount: number;
    platform_withdraw_fee_bps: number;
    platform_withdraw_fee_amount: number;
    provider_withdraw_fee_amount: number;
    net_disbursed_amount: number;
    withdrawable_balance: number;
    provider_partner_ref_no: string;
    provider_inquiry_id: number;
}

export interface PayoutListRow {
    id: string;
    store_id: string;
    requested_amount: number;
    platform_withdraw_fee_amount: number;
    provider_withdraw_fee_amount: number;
    net_disbursed_amount: number;
    status: string;
    bank_name: string;
    account_number_last4: string;
    account_holder_name: string;
    created_at: string;
}

export interface PayoutRecord {
    id: string;
    store_id: string;
    bank_account_id: string;
    requested_by_user_id: string;
    requested_amount: number;
    platform_withdraw_fee_bps: number;
    platform_withdraw_fee_amount: number;
    provider_withdraw_fee_amount: number;
    net_disbursed_amount: number;
    provider_partner_ref_no: string;
    provider_inquiry_id: number;
    status: string;
    failure_reason?: string | null;
    provider_transaction_date?: string | null;
    processed_at?: string | null;
    created_at: string;
    updated_at: string;
}

export interface StoreListResponse {
    stores: Store[];
    total: number;
    page: number;
    per_page: number;
}

export const storesApi = {
    list: async (params: { page?: number; perPage?: number; search?: string; status?: string }) => {
        const query = new URLSearchParams();
        if (params.page) query.append('page', params.page.toString());
        if (params.perPage) query.append('per_page', params.perPage.toString());
        if (params.search) query.append('search', params.search);
        if (params.status) query.append('status', params.status);

        return client.get<StoreListResponse>(`/api/v1/stores?${query.toString()}`);
    },

    get: async (id: string) => {
        return client.get<Store>(`/api/v1/stores/${id}`);
    },

    create: async (data: {
        owner_email?: string;
        owner_user_id?: string;
        name: string;
        slug: string;
        callback_url?: string;
        provider_username: string;
    }) => {
        return client.post<Store>('/api/v1/stores', data);
    },

    update: async (id: string, data: Partial<{
        owner_email: string;
        owner_user_id: string;
        name: string;
        slug: string;
        status: 'active' | 'inactive';
        callback_url: string;
        provider_username: string;
    }>) => {
        return client.put<Store>(`/api/v1/stores/${id}`, data);
    },

    listMembers: async (id: string) => {
        return client.get<{ members: StoreMember[] }>(`/api/v1/stores/${id}/members`);
    },

    addMember: async (
        id: string,
        data: {
            user_email?: string;
            user_id?: string;
            store_role: 'owner' | 'manager' | 'staff' | 'viewer';
        }
    ) => {
        return client.post<StoreMember>(`/api/v1/stores/${id}/members`, data);
    },

    updateMember: async (
        id: string,
        memberId: string,
        data: Partial<{
            store_role: 'owner' | 'manager' | 'staff' | 'viewer';
            status: 'active' | 'inactive';
        }>
    ) => {
        return client.put<StoreMember>(`/api/v1/stores/${id}/members/${memberId}`, data);
    },

    removeMember: async (id: string, memberId: string) => {
        return client.delete(`/api/v1/stores/${id}/members/${memberId}`);
    },

    listTokens: async (id: string) => {
        return client.get<{ tokens: StoreApiToken[] }>(`/api/v1/stores/${id}/tokens`);
    },

    listBanks: async (id: string) => {
        return client.get<{ banks: StoreBankAccount[] }>(`/api/v1/stores/${id}/banks`);
    },

    inquireBank: async (
        id: string,
        data: {
            bank_code: string;
            account_number: string;
        }
    ) => {
        return client.post<{ inquiry: StoreBankInquiry }>(`/api/v1/stores/${id}/banks/inquiry`, data);
    },

    createBank: async (
        id: string,
        data: {
            bank_code: string;
            account_number: string;
            is_default?: boolean;
        }
    ) => {
        return client.post<{ bank: StoreBankAccount }>(`/api/v1/stores/${id}/banks`, data);
    },

    setDefaultBank: async (id: string, bankId: string) => {
        return client.post<{ bank: StoreBankAccount }>(`/api/v1/stores/${id}/banks/${bankId}/default`);
    },

    createToken: async (id: string, data: { name: string }) => {
        return client.post<{ token: StoreApiToken; plaintext_token: string }>(
            `/api/v1/stores/${id}/tokens`,
            data
        );
    },

    revokeToken: async (id: string, tokenId: string) => {
        return client.delete(`/api/v1/stores/${id}/tokens/${tokenId}`);
    },

    getBalances: async (id: string) => {
        return client.get<{ balance: StoreBalanceSnapshot }>(`/api/v1/stores/${id}/balances`);
    },

    previewPayout: async (
        id: string,
        data: {
            bank_account_id: string;
            requested_amount: number;
        }
    ) => {
        return client.post<{ preview: PayoutPreview }>(`/api/v1/stores/${id}/payouts/preview`, data);
    },

    confirmPayout: async (
        id: string,
        data: {
            bank_account_id: string;
            requested_amount: number;
        }
    ) => {
        return client.post<{ payout: PayoutRecord }>(`/api/v1/stores/${id}/payouts`, data);
    },

    listPayouts: async (id: string, params?: { limit?: number; offset?: number }) => {
        const query = new URLSearchParams();
        if (params?.limit) query.append('limit', params.limit.toString());
        if (params?.offset) query.append('offset', params.offset.toString());
        const qs = query.toString();
        return client.get<{ payouts: PayoutListRow[] }>(`/api/v1/stores/${id}/payouts${qs ? `?${qs}` : ''}`);
    },

    getPayoutDetail: async (id: string, payoutId: string) => {
        return client.get<{ payout: PayoutRecord }>(`/api/v1/stores/${id}/payouts/${payoutId}`);
    }
};
