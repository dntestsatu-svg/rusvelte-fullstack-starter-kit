import { client } from './client';

export interface AuthUser {
    id: string;
    name: string;
    email: string;
    role: 'admin' | 'user' | 'dev' | 'superadmin';
    status: 'active' | 'inactive' | 'suspended';
}

export interface LoginResponse {
    user: AuthUser;
    csrf_token: string;
    session_id: string;
}

export const authApi = {
    login: async (data: { email: string; password: string; captcha_token: string }) => {
        return client.post<LoginResponse>('/api/v1/auth/login', data);
    },
    
    logout: async () => {
        return client.post<void>('/api/v1/auth/logout');
    },
    
    me: async () => {
        return client.get<AuthUser>('/api/v1/auth/me');
    }
};
