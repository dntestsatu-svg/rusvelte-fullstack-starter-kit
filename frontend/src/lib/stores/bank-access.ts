export interface StoreBankAccessParams {
	storeOwnerUserId?: string | null;
	sessionUserId?: string | null;
	sessionUserRole?: 'admin' | 'user' | 'dev' | 'superadmin' | null;
}

export interface DefaultableBankRecord {
	id: string;
	is_default: boolean;
}

export function canViewStoreBanks({
	storeOwnerUserId,
	sessionUserId,
	sessionUserRole
}: StoreBankAccessParams): boolean {
	return (
		sessionUserRole === 'dev' ||
		sessionUserRole === 'superadmin' ||
		Boolean(storeOwnerUserId && sessionUserId === storeOwnerUserId)
	);
}

export function canManageStoreBanks({
	storeOwnerUserId,
	sessionUserId,
	sessionUserRole
}: StoreBankAccessParams): boolean {
	return sessionUserRole === 'dev' || Boolean(storeOwnerUserId && sessionUserId === storeOwnerUserId);
}

export function mergeSavedBankRecord<T extends DefaultableBankRecord>(
	existingBanks: T[],
	savedBank: T
): T[] {
	const remainingBanks = existingBanks.filter((bank) => bank.id !== savedBank.id);

	return [
		savedBank,
		...remainingBanks.map((bank) =>
			savedBank.is_default ? ({ ...bank, is_default: false } as T) : bank
		)
	];
}

export function applyDefaultBankRecord<T extends DefaultableBankRecord>(
	existingBanks: T[],
	defaultBankId: string
): T[] {
	return existingBanks.map((bank) => ({ ...bank, is_default: bank.id === defaultBankId }) as T);
}

export function shouldDefaultNewBank(existingBankCount: number): boolean {
	return existingBankCount === 0;
}

export function formatMaskedBankAccount(last4: string): string {
	return `**** ${last4}`;
}
