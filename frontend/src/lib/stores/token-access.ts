export interface StoreTokenAccessParams {
    storeOwnerUserId?: string | null;
    sessionUserId?: string | null;
    sessionUserRole?: 'admin' | 'user' | 'dev' | 'superadmin' | null;
}

export function canAccessStoreTokens({
    storeOwnerUserId,
    sessionUserId,
    sessionUserRole
}: StoreTokenAccessParams): boolean {
    return sessionUserRole === 'dev' || Boolean(storeOwnerUserId && sessionUserId === storeOwnerUserId);
}

export function formatStoreTokenTimestamp(value?: string | null): string {
    if (!value) {
        return 'Never';
    }

    return new Date(value).toLocaleString();
}
