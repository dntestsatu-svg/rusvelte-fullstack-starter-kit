<script lang="ts">
	import { createQuery } from '@tanstack/svelte-query';
	import { page } from '$app/state';
	import { ApiError } from '$lib/api/client';
	import { paymentsApi, type PaymentStatus } from '$lib/api/payments';
	import { paymentQueryKeys } from '$lib/realtime/query-keys';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { Input } from '$lib/components/ui/input';
	import {
		Select,
		SelectContent,
		SelectItem,
		SelectTrigger
	} from '$lib/components/ui/select';
	import {
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow
	} from '$lib/components/ui/table';
	import { ChevronLeft, ChevronRight, Filter, Search, SquarePen } from '@lucide/svelte';

	let currentPage = $state(1);
	let pageSize = 10;
	let statusFilter = $state<'all' | PaymentStatus>('all');
	let searchInput = $state('');
	let searchTerm = $state('');
	let searchDebounce: ReturnType<typeof setTimeout> | null = null;

	const paymentsQuery = createQuery(() => ({
		queryKey: paymentQueryKeys.list({
			page: currentPage,
			perPage: pageSize,
			search: searchTerm,
			status: statusFilter
		}),
		queryFn: () =>
			paymentsApi.list({
				page: currentPage,
				perPage: pageSize,
				search: searchTerm || undefined,
				status: statusFilter === 'all' ? undefined : statusFilter
			})
	}));

	const total = $derived(paymentsQuery.data?.total ?? 0);
	const payments = $derived(paymentsQuery.data?.payments ?? []);
	const totalPages = $derived(Math.ceil(total / pageSize) || 1);
	const isUnauthorized = $derived(
		paymentsQuery.error instanceof ApiError && paymentsQuery.error.status === 403
	);
	const loadError = $derived(
		paymentsQuery.error instanceof ApiError && paymentsQuery.error.status !== 403
			? paymentsQuery.error.message
			: paymentsQuery.error instanceof Error
				? paymentsQuery.error.message
				: ''
	);
	const sessionUser = $derived(page.data.sessionUser);

	function formatMoney(amount: number): string {
		return new Intl.NumberFormat('id-ID').format(amount);
	}

	function handleStatusChange(value: string) {
		statusFilter = value as 'all' | PaymentStatus;
		currentPage = 1;
	}

	function handleSearchInput(event: Event) {
		searchInput = (event.currentTarget as HTMLInputElement).value;
		currentPage = 1;
		if (searchDebounce) {
			clearTimeout(searchDebounce);
		}

		searchDebounce = setTimeout(() => {
			searchTerm = searchInput.trim();
		}, 250);
	}

	function handlePageChange(nextPage: number) {
		if (nextPage < 1 || nextPage > totalPages) {
			return;
		}

		currentPage = nextPage;
	}
</script>

<div class="space-y-6">
	<div class="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
		<div>
			<h1 class="text-3xl font-bold tracking-tight">Payments</h1>
			<p class="text-muted-foreground">Review QRIS payment lifecycle, fees, and scoped store activity.</p>
		</div>
		<div class="rounded-lg border bg-muted/30 px-4 py-2 text-sm text-muted-foreground">
			Signed in as <span class="font-medium text-foreground">{sessionUser?.name}</span>
		</div>
	</div>

	<Card>
		<CardHeader>
			<CardTitle>Payment Activity</CardTitle>
			<CardDescription>
				List updates in realtime through SSE. Filter by status or search by store, provider transaction,
				merchant order, and custom reference.
			</CardDescription>
		</CardHeader>
		<CardContent class="space-y-4">
			<div class="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
				<div class="relative max-w-sm flex-1">
					<Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
					<Input
						type="search"
						class="pl-8"
						placeholder="Search payments..."
						value={searchInput}
						oninput={handleSearchInput}
					/>
				</div>

				<div class="flex items-center gap-2">
					<Filter class="h-4 w-4 text-muted-foreground" />
					<Select type="single" value={statusFilter} onValueChange={(value) => handleStatusChange(value)}>
						<SelectTrigger class="w-[180px]">
							<span class="capitalize">{statusFilter === 'all' ? 'All statuses' : statusFilter}</span>
						</SelectTrigger>
						<SelectContent>
							<SelectItem value="all">All Statuses</SelectItem>
							<SelectItem value="created">Created</SelectItem>
							<SelectItem value="pending">Pending</SelectItem>
							<SelectItem value="success">Success</SelectItem>
							<SelectItem value="failed">Failed</SelectItem>
							<SelectItem value="expired">Expired</SelectItem>
						</SelectContent>
					</Select>
				</div>
			</div>

			<div class="rounded-md border">
				<Table>
					<TableHeader>
						<TableRow>
							<TableHead>Store</TableHead>
							<TableHead>Status</TableHead>
							<TableHead>Gross</TableHead>
							<TableHead>Pending Credit</TableHead>
							<TableHead>Reference</TableHead>
							<TableHead>Created</TableHead>
							<TableHead class="text-right">Detail</TableHead>
						</TableRow>
					</TableHeader>
					<TableBody>
						{#if paymentsQuery.isPending}
							<TableRow>
								<TableCell colspan={7} class="h-24 text-center">Loading payments...</TableCell>
							</TableRow>
						{:else if isUnauthorized}
							<TableRow>
								<TableCell colspan={7} class="h-24 text-center">
									You do not have permission to view payments.
								</TableCell>
							</TableRow>
						{:else if loadError}
							<TableRow>
								<TableCell colspan={7} class="h-24 text-center">
									<div class="space-y-3">
										<p class="text-destructive">{loadError}</p>
										<Button variant="outline" size="sm" onclick={() => void paymentsQuery.refetch()}>
											Try Again
										</Button>
									</div>
								</TableCell>
							</TableRow>
						{:else if payments.length === 0}
							<TableRow>
								<TableCell colspan={7} class="h-24 text-center">
									No payments found for this scope.
								</TableCell>
							</TableRow>
						{:else}
							{#each payments as paymentRow}
								<TableRow>
									<TableCell>
										<div class="flex flex-col">
											<span class="font-medium">{paymentRow.store_name}</span>
											<span class="text-sm text-muted-foreground">{paymentRow.store_slug}</span>
										</div>
									</TableCell>
									<TableCell>
										<Badge
											variant={paymentRow.status === 'success'
												? 'default'
												: paymentRow.status === 'pending'
													? 'outline'
													: paymentRow.status === 'created'
														? 'secondary'
														: 'destructive'}
											class="capitalize"
										>
											{paymentRow.status}
										</Badge>
									</TableCell>
									<TableCell>Rp {formatMoney(paymentRow.gross_amount)}</TableCell>
									<TableCell>Rp {formatMoney(paymentRow.store_pending_credit_amount)}</TableCell>
									<TableCell>
										<div class="flex flex-col">
											<span class="font-medium">
												{paymentRow.merchant_order_id ?? paymentRow.provider_trx_id ?? 'No external ref'}
											</span>
											{#if paymentRow.custom_ref}
												<span class="text-sm text-muted-foreground">{paymentRow.custom_ref}</span>
											{/if}
										</div>
									</TableCell>
									<TableCell>{new Date(paymentRow.created_at).toLocaleString()}</TableCell>
									<TableCell class="text-right">
										<Button variant="ghost" size="icon" href={`/dashboard/payments/${paymentRow.id}`}>
											<SquarePen class="h-4 w-4" />
										</Button>
									</TableCell>
								</TableRow>
							{/each}
						{/if}
					</TableBody>
				</Table>
			</div>

			<div class="flex items-center justify-between">
				<p class="text-sm text-muted-foreground">
					Showing {payments.length} of {total} payments
				</p>
				<div class="flex items-center gap-2">
					<Button
						variant="outline"
						size="sm"
						disabled={currentPage === 1 || paymentsQuery.isPending}
						onclick={() => handlePageChange(currentPage - 1)}
					>
						<ChevronLeft class="h-4 w-4" />
						Previous
					</Button>
					<div class="text-sm font-medium">Page {currentPage} of {totalPages}</div>
					<Button
						variant="outline"
						size="sm"
						disabled={currentPage >= totalPages || paymentsQuery.isPending}
						onclick={() => handlePageChange(currentPage + 1)}
					>
						Next
						<ChevronRight class="h-4 w-4" />
					</Button>
				</div>
			</div>
		</CardContent>
	</Card>
</div>
