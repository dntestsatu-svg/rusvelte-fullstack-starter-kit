import { client } from './client';

export interface User {
    id: string;
    name: string;
    email: string;
    role: 'dev' | 'superadmin' | 'admin' | 'user';
    status: 'active' | 'inactive' | 'suspended';
    created_at: string;
    updated_at: string;
    last_login_at?: string;
}

export interface UserListResponse {
    users: User[];
    total: number;
    page: number;
    per_page: number;
}

export const usersApi = {
    list: async (params: { page?: number; perPage?: number; role?: string; search?: string }) => {
        const query = new URLSearchParams();
        if (params.page) query.append('page', params.page.toString());
        if (params.perPage) query.append('per_page', params.perPage.toString());
        if (params.role) query.append('role', params.role);
        if (params.search) query.append('search', params.search);
        
        return client.get<UserListResponse>(`/api/v1/users?${query.toString()}`);
    },
    
    get: async (id: string) => {
        return client.get<User>(`/api/v1/users/${id}`);
    },
    
    create: async (data: any) => {
        return client.post<User>('/api/v1/users', data);
    },
    
    update: async (id: string, data: any) => {
        return client.put<User>(`/api/v1/users/${id}`, data);
    },
    
    disable: async (id: string) => {
        return client.post(`/api/v1/users/${id}/disable`, {});
    }
};
