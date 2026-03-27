export interface StoreOption {
	id: string;
	name: string;
}

export interface StoreSelection {
	stores: StoreOption[];
	selectedStoreId: string;
	selectedStoreName: string;
	requiresSelection: boolean;
	hasMultiple: boolean;
}

export function deriveStoreSelection(
	stores: StoreOption[] | undefined,
	selectedId: string
): StoreSelection {
	const safeStores = stores ?? [];

	if (safeStores.length === 0) {
		return {
			stores: safeStores,
			selectedStoreId: '',
			selectedStoreName: '',
			requiresSelection: false,
			hasMultiple: false
		};
	}

	if (safeStores.length === 1) {
		return {
			stores: safeStores,
			selectedStoreId: safeStores[0].id,
			selectedStoreName: safeStores[0].name,
			requiresSelection: false,
			hasMultiple: false
		};
	}

	const matched = safeStores.find((store) => store.id === selectedId);
	if (!matched) {
		return {
			stores: safeStores,
			selectedStoreId: '',
			selectedStoreName: '',
			requiresSelection: true,
			hasMultiple: true
		};
	}

	return {
		stores: safeStores,
		selectedStoreId: matched.id,
		selectedStoreName: matched.name,
		requiresSelection: false,
		hasMultiple: true
	};
}
