<script lang="ts">
	import { createQuery } from '@tanstack/svelte-query';
	import { page } from '$app/state';
	import { ApiError } from '$lib/api/client';
	import { dashboardApi } from '$lib/api/dashboard';
	import { paymentsApi, type DashboardPaymentDistribution } from '$lib/api/payments';
	import { storesApi, type StoreBalanceSnapshot } from '$lib/api/stores';
	import { deriveStoreSelection } from '$lib/dashboard/store-selection';
	import DashboardOverview, {
		type BalanceCard,
		type PaymentDistribution
	} from '$lib/components/dashboard/DashboardOverview.svelte';
	import {
		Select,
		SelectContent,
		SelectItem,
		SelectTrigger
	} from '$lib/components/ui/select';
	import { balanceQueryKeys, paymentQueryKeys } from '$lib/realtime/query-keys';

	const sessionUser = $derived(page.data.sessionUser);
	const isDev = $derived(sessionUser?.role === 'dev');
	let selectedStoreId = $state('');

	const storeListQuery = createQuery(() => ({
		queryKey: ['stores', 'list', 'dashboard', sessionUser?.role ?? 'unknown'],
		queryFn: () => storesApi.list({ page: 1, perPage: 50 }),
		enabled: !isDev
	}));

	const storeOptions = $derived(storeListQuery.data?.stores ?? []);
	const storeSelection = $derived(deriveStoreSelection(storeOptions, selectedStoreId));
	const storeId = $derived(storeSelection.selectedStoreId);
	const storeName = $derived(storeSelection.selectedStoreName);

	$effect(() => {
		if (isDev) return;
		if (storeOptions.length === 1 && selectedStoreId !== storeOptions[0].id) {
			selectedStoreId = storeOptions[0].id;
		}
	});

	const storeBalanceQuery = createQuery(() => ({
		queryKey: storeId ? balanceQueryKeys.store(storeId) : balanceQueryKeys.all,
		queryFn: () => storesApi.getBalances(storeId),
		enabled: !isDev && Boolean(storeId)
	}));

	const providerBalanceQuery = createQuery(() => ({
		queryKey: ['provider-balance'],
		queryFn: () => dashboardApi.getProviderBalance(),
		enabled: isDev
	}));

	const distributionQuery = createQuery(() => ({
		queryKey: paymentQueryKeys.distribution(),
		queryFn: () => paymentsApi.getDistribution()
	}));

	const balanceUnauthorized = $derived((() => {
		const error = isDev ? providerBalanceQuery.error : storeBalanceQuery.error;
		return error instanceof ApiError && error.status === 403;
	})());

	const balanceErrorMessage = $derived((() => {
		const error = isDev ? providerBalanceQuery.error : storeBalanceQuery.error;
		if (error instanceof ApiError) {
			return error.message;
		}
		if (error instanceof Error) {
			return error.message;
		}
		if (storeListQuery.error instanceof Error) {
			return storeListQuery.error.message;
		}
		return '';
	})());

	const balanceState = $derived((() => {
		if (isDev) {
			if (providerBalanceQuery.isPending) return 'loading';
			if (providerBalanceQuery.error) return balanceUnauthorized ? 'unauthorized' : 'error';
			return 'ready';
		}

		if (storeListQuery.isPending) return 'loading';
		if (storeListQuery.error) return 'error';
		if (storeOptions.length === 0) return 'empty';
		if (storeSelection.requiresSelection) return 'empty';
		if (!storeId) return 'empty';
		if (storeBalanceQuery.isPending) return 'loading';
		if (storeBalanceQuery.error) return balanceUnauthorized ? 'unauthorized' : 'error';
		return 'ready';
	})());

	const distributionState = $derived((() => {
		if (distributionQuery.isPending) return 'loading';
		if (distributionQuery.error) return 'error';
		const distribution = distributionQuery.data?.distribution;
		if (!distribution) return 'empty';
		const total = distribution.success + distribution.failed + distribution.expired;
		return total === 0 ? 'empty' : 'ready';
	})());

	const distributionErrorMessage = $derived((() => {
		const error = distributionQuery.error;
		if (error instanceof ApiError) return error.message;
		if (error instanceof Error) return error.message;
		return '';
	})());

	function formatMoney(amount: number): string {
		return new Intl.NumberFormat('id-ID').format(amount);
	}

	function buildStoreCards(balance: StoreBalanceSnapshot): BalanceCard[] {
		return [
			{
				label: 'Pending Balance',
				value: `Rp ${formatMoney(balance.pending_balance)}`,
				description: 'Pending credits from successful payments after the 3% platform fee.',
				tooltip:
					'Pending balance = hasil payment sukses setelah potong fee 3%, belum bisa ditarik.'
			},
			{
				label: 'Settled Balance',
				value: `Rp ${formatMoney(balance.settled_balance)}`,
				description: 'Balance that has been settled by developers and is eligible for withdrawal.',
				tooltip: 'Settled balance = saldo yang sudah disettle manual oleh developer.'
			},
			{
				label: 'Withdrawable Balance',
				value: `Rp ${formatMoney(balance.withdrawable_balance)}`,
				description: 'Amount currently available for payout after reserves.',
				tooltip: 'Withdrawable balance = settled - reserved.'
			}
		];
	}

	function buildProviderCards(snapshot: {
		provider_pending_balance: number;
		provider_settle_balance: number;
	}): BalanceCard[] {
		return [
			{
				label: 'Provider Pending Balance',
				value: `Rp ${formatMoney(snapshot.provider_pending_balance)}`,
				description: 'Monitoring view of pending funds reported by the provider.',
				tooltip: 'Provider pending balance hanya untuk monitoring, bukan saldo store.'
			},
			{
				label: 'Provider Settled Balance',
				value: `Rp ${formatMoney(snapshot.provider_settle_balance)}`,
				description: 'Monitoring view of settled funds reported by the provider.',
				tooltip: 'Provider settled balance hanya untuk monitoring, bukan saldo store.'
			}
		];
	}

	const balanceCards = $derived((() => {
		if (balanceState !== 'ready') return [];
		if (isDev) {
			const snapshot = providerBalanceQuery.data;
			return snapshot ? buildProviderCards(snapshot) : [];
		}
		const balance = storeBalanceQuery.data?.balance;
		return balance ? buildStoreCards(balance) : [];
	})());

	const distribution = $derived<PaymentDistribution>((() => {
		const data = distributionQuery.data?.distribution as DashboardPaymentDistribution | undefined;
		return {
			success: data?.success ?? 0,
			failed: data?.failed ?? 0,
			expired: data?.expired ?? 0
		};
	})());

	const balanceTitle = $derived(
		(isDev ? 'Provider Balance (Monitoring)' : 'Store Balance Overview')
	);
	const storeScopeMessage = $derived((() => {
		if (storeListQuery.isPending) return 'Loading available stores...';
		if (storeOptions.length === 0) return 'No store assigned yet.';
		if (storeSelection.requiresSelection) return 'Select a store to view balances.';
		return storeName ? `Active store: ${storeName}` : 'Select a store to view balances.';
	})());
	const showStoreSelector = $derived(
		!isDev && (storeSelection.hasMultiple || storeListQuery.isPending)
	);
	const balanceSubtitle = $derived(
		isDev
			? 'External provider balances for reconciliation only.'
			: storeSelection.requiresSelection
				? 'Select a store to view balances.'
				: storeName
					? `Showing ${storeName}`
					: ''
	);
	const balancePlaceholderCount = $derived((isDev ? 2 : 3));

	function retryBalance() {
		if (isDev) {
			void providerBalanceQuery.refetch();
			return;
		}

		void storeListQuery.refetch();
		if (storeId) {
			void storeBalanceQuery.refetch();
		}
	}

	function retryDistribution() {
		void distributionQuery.refetch();
	}
</script>

<div class="space-y-6">
	<div class="flex items-center justify-between">
		<div>
			<h1 class="text-3xl font-bold tracking-tight">Dashboard Overview</h1>
			<p class="text-muted-foreground mt-1">Welcome back to JustQiu Merchant Portal.</p>
		</div>
	</div>

	{#if !isDev}
		<div class="flex flex-col gap-3 rounded-xl border border-border/50 bg-card/40 px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
			<div>
				<p class="text-sm font-medium">Store Balance Scope</p>
				<p class="text-xs text-muted-foreground">{storeScopeMessage}</p>
			</div>
			{#if showStoreSelector}
				<Select
					type="single"
					value={selectedStoreId}
					onValueChange={(value) => {
						selectedStoreId = value;
					}}
				>
					<SelectTrigger class="w-[220px]">
						<span>{storeSelection.requiresSelection ? 'Select a store' : storeName}</span>
					</SelectTrigger>
					<SelectContent>
						{#each storeOptions as store}
							<SelectItem value={store.id}>{store.name}</SelectItem>
						{/each}
					</SelectContent>
				</Select>
			{/if}
		</div>
	{/if}

	<DashboardOverview
		balanceTitle={balanceTitle}
		balanceSubtitle={balanceSubtitle}
		balanceState={balanceState}
		balanceCards={balanceCards}
		balanceErrorMessage={balanceErrorMessage}
		balanceEmptyMessage={
			storeSelection.requiresSelection
				? 'Select a store to view balances.'
				: 'No stores found for this account yet.'
		}
		balanceUnauthorizedMessage="You do not have permission to view balances for this store."
		balancePlaceholderCount={balancePlaceholderCount}
		balanceRetry={retryBalance}
		distributionState={distributionState}
		distribution={distribution}
		distributionErrorMessage={distributionErrorMessage}
		distributionRetry={retryDistribution}
		distributionEmptyMessage="Payment status distribution will appear after finalized transactions."
	/>
</div>
