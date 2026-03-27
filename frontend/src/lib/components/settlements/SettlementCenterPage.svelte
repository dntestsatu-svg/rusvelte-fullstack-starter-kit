<script lang="ts">
	import { createMutation, createQuery, useQueryClient } from '@tanstack/svelte-query';
	import { page } from '$app/state';
	import { ApiError } from '$lib/api/client';
	import { settlementsApi, type SettlementProcessResponse } from '$lib/api/settlements';
	import { storesApi } from '$lib/api/stores';
	import { deriveStoreSelection } from '$lib/dashboard/store-selection';
	import SettlementCenterView from '$lib/components/settlements/SettlementCenterView.svelte';
	import { balanceQueryKeys } from '$lib/realtime/query-keys';

	const sessionUser = $derived(page.data.sessionUser);
	const isDev = $derived(sessionUser?.role === 'dev');
	const queryClient = useQueryClient();

	let selectedStoreId = $state('');
	let amountValue = $state('');
	let notesValue = $state('');
	let submitErrorMessage = $state('');
	let receipt = $state<SettlementProcessResponse | null>(null);

	const storesQuery = createQuery(() => ({
		queryKey: ['stores', 'list', 'settlement-center'],
		queryFn: () => storesApi.list({ page: 1, perPage: 50 }),
		enabled: isDev
	}));

	const storeOptions = $derived(
		(storesQuery.data?.stores ?? []).map((store) => ({ id: store.id, name: store.name }))
	);
	const storeSelection = $derived(deriveStoreSelection(storeOptions, selectedStoreId));
	const activeStoreId = $derived(storeSelection.selectedStoreId);
	const activeStoreName = $derived(storeSelection.selectedStoreName);

	$effect(() => {
		if (!isDev) return;
		if (storeOptions.length === 1 && selectedStoreId !== storeOptions[0].id) {
			selectedStoreId = storeOptions[0].id;
		}
	});

	const balanceQuery = createQuery(() => ({
		queryKey: activeStoreId ? balanceQueryKeys.store(activeStoreId) : balanceQueryKeys.all,
		queryFn: () => storesApi.getBalances(activeStoreId),
		enabled: isDev && Boolean(activeStoreId) && !storeSelection.requiresSelection
	}));

	const settlementMutation = createMutation(() => ({
		mutationFn: (payload: { store_id: string; amount: number; notes?: string }) =>
			settlementsApi.create(payload),
		onSuccess: async (response) => {
			receipt = response;
			amountValue = '';
			notesValue = '';
			submitErrorMessage = '';
			await queryClient.invalidateQueries({ queryKey: balanceQueryKeys.all });
		}
	}));

	const storeState = $derived((() => {
		if (!isDev) return 'empty';
		if (storesQuery.isPending) return 'loading';
		if (storesQuery.error) return 'error';
		return storeOptions.length === 0 ? 'empty' : 'ready';
	})());

	const balanceState = $derived((() => {
		if (!isDev) return 'idle';
		if (storeState !== 'ready') return 'idle';
		if (storeSelection.requiresSelection) return 'idle';
		if (balanceQuery.isPending) return 'loading';
		if (balanceQuery.error) return 'error';
		return balanceQuery.data?.balance ? 'ready' : 'empty';
	})());

	const storeErrorMessage = $derived((() => {
		const error = storesQuery.error;
		if (error instanceof ApiError) return error.message;
		if (error instanceof Error) return error.message;
		return '';
	})());

	const balanceErrorMessage = $derived((() => {
		const error = balanceQuery.error;
		if (error instanceof ApiError) return error.message;
		if (error instanceof Error) return error.message;
		return '';
	})());

	const submitState = $derived((() => {
		if (settlementMutation.isPending) return 'submitting';
		if (submitErrorMessage) return 'error';
		if (receipt) return 'success';
		return 'idle';
	})());

	const currentBalance = $derived(
		receipt?.balance.store_id === activeStoreId
			? receipt.balance
			: balanceQuery.data?.balance
	);

	async function handleSubmit() {
		receipt = null;
		submitErrorMessage = '';

		if (!activeStoreId || storeSelection.requiresSelection) {
			submitErrorMessage = 'Select a store before submitting settlement.';
			return;
		}

		const parsedAmount = Number.parseInt(amountValue, 10);
		if (!Number.isInteger(parsedAmount) || parsedAmount <= 0) {
			submitErrorMessage = 'Enter a positive whole Rupiah amount.';
			return;
		}

		try {
			await settlementMutation.mutateAsync({
				store_id: activeStoreId,
				amount: parsedAmount,
				notes: notesValue.trim() ? notesValue.trim() : undefined
			});
		} catch (error) {
			submitErrorMessage =
				error instanceof ApiError
					? error.message
					: error instanceof Error
						? error.message
						: 'Settlement could not be processed.';
		}
	}
</script>

<SettlementCenterView
	isDev={isDev}
	storeState={storeState}
	storeOptions={storeOptions}
	selectedStoreId={selectedStoreId}
	activeStoreName={activeStoreName}
	requiresSelection={storeSelection.requiresSelection}
	storeErrorMessage={storeErrorMessage}
	balanceState={balanceState}
	balance={currentBalance
		? {
				pending_balance: currentBalance.pending_balance,
				settled_balance: currentBalance.settled_balance,
				withdrawable_balance: currentBalance.withdrawable_balance
			}
		: null}
	balanceErrorMessage={balanceErrorMessage}
	amountValue={amountValue}
	notesValue={notesValue}
	submitState={submitState}
	submitErrorMessage={submitErrorMessage}
	receipt={receipt
		? {
				id: receipt.settlement.id,
				amount: receipt.settlement.amount,
				storeName: activeStoreName || 'Selected store',
				pending_balance: receipt.balance.pending_balance,
				settled_balance: receipt.balance.settled_balance,
				withdrawable_balance: receipt.balance.withdrawable_balance,
				created_at: receipt.settlement.created_at
			}
		: null}
	onStoreChange={(value) => {
		selectedStoreId = value;
		receipt = null;
		submitErrorMessage = '';
	}}
	onAmountInput={(value) => {
		amountValue = value;
		receipt = null;
		submitErrorMessage = '';
	}}
	onNotesInput={(value) => {
		notesValue = value;
		receipt = null;
	}}
	onSubmit={handleSubmit}
/>
