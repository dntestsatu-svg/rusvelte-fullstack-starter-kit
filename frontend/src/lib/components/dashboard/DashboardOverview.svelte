<script module lang="ts">
	export interface BalanceCard {
		label: string;
		value: string;
		description: string;
		tooltip: string;
		tone?: 'default' | 'accent';
	}

	export interface PaymentDistribution {
		success: number;
		failed: number;
		expired: number;
	}
</script>

<script lang="ts">
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
	import ErrorState from '$lib/components/ui/ErrorState.svelte';
	import EmptyState from '$lib/components/ui/EmptyState.svelte';
	import { Skeleton } from '$lib/components/ui/skeleton';
	import { Info } from '@lucide/svelte';

	let {
		balanceTitle = 'Balance Overview',
		balanceSubtitle = '',
		balanceState = 'loading',
		balanceCards = [],
		balanceErrorMessage = '',
		balanceEmptyMessage = 'No store balance data available yet.',
		balanceUnauthorizedMessage = 'You do not have permission to view balances.',
		balancePlaceholderCount = 3,
		balanceRetry = null,
		distributionState = 'loading',
		distribution = { success: 0, failed: 0, expired: 0 },
		distributionErrorMessage = '',
		distributionEmptyMessage = 'No finalized payments yet.',
		distributionRetry = null
	} = $props();

	const chartItems = $derived([
		{ label: 'Success', value: distribution.success, tone: 'bg-emerald-500/80' },
		{ label: 'Failed', value: distribution.failed, tone: 'bg-rose-500/80' },
		{ label: 'Expired', value: distribution.expired, tone: 'bg-amber-500/80' }
	]);

	const chartMax = $derived(
		Math.max(distribution.success, distribution.failed, distribution.expired, 1)
	);
</script>

<div class="space-y-6">
	<div class="flex items-center justify-between">
		<div>
			<h2 class="text-lg font-semibold">{balanceTitle}</h2>
			{#if balanceSubtitle}
				<p class="text-sm text-muted-foreground mt-1">{balanceSubtitle}</p>
			{/if}
		</div>
	</div>

	{#if balanceState === 'loading'}
		<div class="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
			{#each Array(balancePlaceholderCount) as _, index (index)}
				<div class="p-6 bg-card rounded-xl border border-border/50 shadow-sm space-y-2">
					<Skeleton class="h-4 w-32" />
					<Skeleton class="h-8 w-24" />
					<Skeleton class="h-3 w-40" />
				</div>
			{/each}
		</div>
	{:else if balanceState === 'error'}
		<ErrorState
			title="Balance summary unavailable"
			message={balanceErrorMessage || 'We could not load balance information.'}
			retry={balanceRetry}
		/>
	{:else if balanceState === 'unauthorized'}
		<EmptyState title="Access restricted" message={balanceUnauthorizedMessage} />
	{:else if balanceState === 'empty'}
		<EmptyState title="No balances yet" message={balanceEmptyMessage} />
	{:else}
		<div class="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
			{#each balanceCards as card}
				<div class="p-6 bg-card rounded-xl border border-border/50 shadow-sm space-y-3">
					<div class="flex items-start justify-between">
						<div>
							<p class="text-sm text-muted-foreground">{card.label}</p>
							<p class="text-2xl font-semibold tracking-tight">{card.value}</p>
						</div>
						<div class="flex h-8 w-8 items-center justify-center rounded-full bg-muted/60 text-muted-foreground">
							<Info class="h-4 w-4" title={card.tooltip} />
						</div>
					</div>
					<p class="text-xs text-muted-foreground">{card.description}</p>
				</div>
			{/each}
		</div>
	{/if}

	<Card>
		<CardHeader>
			<CardTitle>Payment Distribution</CardTitle>
		</CardHeader>
		<CardContent>
			{#if distributionState === 'loading'}
				<div class="space-y-4">
					<Skeleton class="h-6 w-32" />
					<Skeleton class="h-28 w-full" />
				</div>
			{:else if distributionState === 'error'}
				<ErrorState
					title="Payment distribution unavailable"
					message={distributionErrorMessage || 'We could not load payment distribution data.'}
					retry={distributionRetry}
				/>
			{:else if distributionState === 'empty'}
				<EmptyState title="No finalized payments" message={distributionEmptyMessage} />
			{:else}
				<div class="grid gap-4 md:grid-cols-3">
					{#each chartItems as item}
						<div class="rounded-xl border border-border/50 bg-muted/30 p-4">
							<div class="flex items-center justify-between text-sm">
								<span class="text-muted-foreground">{item.label}</span>
								<span class="font-semibold text-foreground">{item.value}</span>
							</div>
							<div class="mt-4 h-28 flex items-end">
								<div
									class={`w-full rounded-md ${item.tone}`}
									style={`height: ${Math.max(10, Math.round((item.value / chartMax) * 100))}%`}
								></div>
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</CardContent>
	</Card>
</div>
