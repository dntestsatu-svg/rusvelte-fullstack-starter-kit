<script lang="ts">
	import { createQuery } from '@tanstack/svelte-query';
	import { ApiError } from '$lib/api/client';
	import { paymentsApi } from '$lib/api/payments';
	import { paymentQueryKeys } from '$lib/realtime/query-keys';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { ChevronLeft } from '@lucide/svelte';

	let { paymentId } = $props<{ paymentId: string }>();

	const paymentQuery = createQuery(() => ({
		queryKey: paymentQueryKeys.detail(paymentId),
		queryFn: () => paymentsApi.get(paymentId)
	}));

	const payment = $derived(paymentQuery.data?.payment ?? null);
	const isUnauthorized = $derived(
		paymentQuery.error instanceof ApiError && paymentQuery.error.status === 403
	);
	const loadError = $derived(
		paymentQuery.error instanceof ApiError && paymentQuery.error.status !== 403
			? paymentQuery.error.message
			: paymentQuery.error instanceof Error
				? paymentQuery.error.message
				: ''
	);

	function formatMoney(amount: number): string {
		return new Intl.NumberFormat('id-ID').format(amount);
	}
</script>

<div class="space-y-6">
	<div class="flex items-center gap-4">
		<Button variant="outline" size="icon" href="/dashboard/payments">
			<ChevronLeft class="h-4 w-4" />
		</Button>
		<div>
			<h1 class="text-3xl font-bold tracking-tight">Payment Detail</h1>
			<p class="text-muted-foreground">Inspect provider metadata, fee breakdown, and finalization state.</p>
		</div>
	</div>

	{#if paymentQuery.isPending}
		<div class="rounded-lg border border-dashed p-12 text-center text-muted-foreground">
			Loading payment detail...
		</div>
	{:else if isUnauthorized}
		<div class="rounded-lg border border-dashed p-12 text-center text-muted-foreground">
			You do not have permission to view this payment.
		</div>
	{:else if loadError}
		<div class="rounded-lg border border-dashed p-12 text-center text-muted-foreground">
			<div class="space-y-3">
				<p class="text-destructive">{loadError}</p>
				<Button variant="outline" size="sm" onclick={() => void paymentQuery.refetch()}>Try Again</Button>
			</div>
		</div>
	{:else if payment}
		<div class="grid gap-6 xl:grid-cols-[1.15fr_0.85fr]">
			<Card>
				<CardHeader>
					<div class="flex items-start justify-between gap-4">
						<div>
							<CardTitle>{payment.store_name}</CardTitle>
							<CardDescription>{payment.store_slug}</CardDescription>
						</div>
						<Badge
							variant={payment.status === 'success'
								? 'default'
								: payment.status === 'pending'
									? 'outline'
									: payment.status === 'created'
										? 'secondary'
										: 'destructive'}
							class="capitalize"
						>
							{payment.status}
						</Badge>
					</div>
				</CardHeader>
				<CardContent class="space-y-4 text-sm">
					<div class="grid gap-4 md:grid-cols-2">
						<div class="rounded-lg border bg-muted/30 p-4">
							<p class="text-muted-foreground">Gross Amount</p>
							<p class="mt-2 text-2xl font-semibold">Rp {formatMoney(payment.gross_amount)}</p>
						</div>
						<div class="rounded-lg border bg-muted/30 p-4">
							<p class="text-muted-foreground">Pending Credit</p>
							<p class="mt-2 text-2xl font-semibold">
								Rp {formatMoney(payment.store_pending_credit_amount)}
							</p>
						</div>
					</div>

					<div class="grid gap-3 rounded-lg border bg-muted/20 p-4">
						<div class="flex justify-between gap-4">
							<span class="text-muted-foreground">Payment ID</span>
							<span class="text-right font-mono text-xs">{payment.id}</span>
						</div>
						<div class="flex justify-between gap-4">
							<span class="text-muted-foreground">Provider</span>
							<span>{payment.provider_name}</span>
						</div>
						<div class="flex justify-between gap-4">
							<span class="text-muted-foreground">Provider Terminal</span>
							<span>{payment.provider_terminal_id ?? 'N/A'}</span>
						</div>
						<div class="flex justify-between gap-4">
							<span class="text-muted-foreground">Provider Transaction</span>
							<span>{payment.provider_trx_id ?? 'Pending'}</span>
						</div>
						<div class="flex justify-between gap-4">
							<span class="text-muted-foreground">Provider RRN</span>
							<span>{payment.provider_rrn ?? 'N/A'}</span>
						</div>
						<div class="flex justify-between gap-4">
							<span class="text-muted-foreground">Merchant Order</span>
							<span>{payment.merchant_order_id ?? 'N/A'}</span>
						</div>
						<div class="flex justify-between gap-4">
							<span class="text-muted-foreground">Custom Ref</span>
							<span>{payment.custom_ref ?? 'N/A'}</span>
						</div>
					</div>
				</CardContent>
			</Card>

			<Card>
				<CardHeader>
					<CardTitle>Financial & Timeline</CardTitle>
					<CardDescription>Internal fee breakdown and provider timestamps.</CardDescription>
				</CardHeader>
				<CardContent class="space-y-3 text-sm">
					<div class="flex justify-between gap-4">
						<span class="text-muted-foreground">Platform Fee BPS</span>
						<span>{payment.platform_tx_fee_bps}</span>
					</div>
					<div class="flex justify-between gap-4">
						<span class="text-muted-foreground">Platform Fee Amount</span>
						<span>Rp {formatMoney(payment.platform_tx_fee_amount)}</span>
					</div>
					<div class="flex justify-between gap-4">
						<span class="text-muted-foreground">Created At</span>
						<span>{new Date(payment.created_at).toLocaleString()}</span>
					</div>
					<div class="flex justify-between gap-4">
						<span class="text-muted-foreground">Expired At</span>
						<span>{payment.expired_at ? new Date(payment.expired_at).toLocaleString() : 'N/A'}</span>
					</div>
					<div class="flex justify-between gap-4">
						<span class="text-muted-foreground">Provider Created</span>
						<span>
							{payment.provider_created_at
								? new Date(payment.provider_created_at).toLocaleString()
								: 'N/A'}
						</span>
					</div>
					<div class="flex justify-between gap-4">
						<span class="text-muted-foreground">Provider Finished</span>
						<span>
							{payment.provider_finished_at
								? new Date(payment.provider_finished_at).toLocaleString()
								: 'N/A'}
						</span>
					</div>
					<div class="flex justify-between gap-4">
						<span class="text-muted-foreground">Finalized At</span>
						<span>{payment.finalized_at ? new Date(payment.finalized_at).toLocaleString() : 'N/A'}</span>
					</div>
					<div class="grid gap-2 rounded-lg border bg-muted/30 p-4">
						<span class="text-muted-foreground">QRIS Payload</span>
						<code class="break-all text-xs">{payment.qris_payload ?? 'Unavailable'}</code>
					</div>
				</CardContent>
			</Card>
		</div>
	{:else}
		<div class="rounded-lg border border-dashed p-12 text-center text-muted-foreground">
			Payment not found.
		</div>
	{/if}
</div>
