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

export interface StoreBalanceSnapshot {
    store_id: string;
    pending_balance: number;
    settled_balance: number;
    reserved_settled_balance: number;
    withdrawable_balance: number;
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
    }
};
