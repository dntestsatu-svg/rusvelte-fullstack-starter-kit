<script lang="ts">
	import { onMount } from 'svelte';

	import { ApiError } from '$lib/api/client';
	import {
		storesApi,
		type PayoutListRow,
		type PayoutPreview,
		type StoreBankAccount,
		type StoreBalanceSnapshot
	} from '$lib/api/stores';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
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
	import { ArrowDownToLine, Loader2, RefreshCcw, Send } from '@lucide/svelte';
	import { toast } from 'svelte-sonner';

	let {
		storeId,
		canManage
	}: {
		storeId: string;
		canManage: boolean;
	} = $props();

	let payouts = $state<PayoutListRow[]>([]);
	let banks = $state<StoreBankAccount[]>([]);
	let balance = $state<StoreBalanceSnapshot | null>(null);
	let preview = $state<PayoutPreview | null>(null);

	let selectedBankId = $state('');
	let requestedAmount = $state('');

	let isLoading = $state(true);
	let isPreviewLoading = $state(false);
	let isConfirming = $state(false);
	let errorMessage = $state('');
	let isUnauthorized = $state(false);

	onMount(() => {
		void loadData();
	});

	async function loadData() {
		isLoading = true;
		errorMessage = '';

		try {
			const [payoutsRes, banksRes, balanceRes] = await Promise.all([
				storesApi.listPayouts(storeId),
				storesApi.listBanks(storeId),
				storesApi.getBalances(storeId)
			]);
			payouts = payoutsRes.payouts;
			banks = banksRes.banks;
			balance = balanceRes.balance;
			isUnauthorized = false;
		} catch (error) {
			handlePanelError(error, 'Failed to load payout data');
		} finally {
			isLoading = false;
		}
	}

	async function handlePreview(event: Event) {
		event.preventDefault();
		if (!canManage || !selectedBankId || !requestedAmount) return;

		isPreviewLoading = true;
		preview = null;

		try {
			const res = await storesApi.previewPayout(storeId, {
				bank_account_id: selectedBankId,
				requested_amount: Number(requestedAmount)
			});
			preview = res.preview;
			errorMessage = '';
			isUnauthorized = false;
		} catch (error) {
			preview = null;
			handlePanelError(error, 'Failed to preview payout');
		} finally {
			isPreviewLoading = false;
		}
	}

	async function handleConfirm() {
		if (!canManage || !preview) return;

		isConfirming = true;

		try {
			await storesApi.confirmPayout(storeId, {
				bank_account_id: preview.bank_account_id,
				requested_amount: preview.requested_amount
			});
			preview = null;
			requestedAmount = '';
			selectedBankId = '';
			toast.success('Payout confirmed and submitted');
			await loadData();
		} catch (error) {
			handlePanelError(error, 'Failed to confirm payout');
		} finally {
			isConfirming = false;
		}
	}

	function formatRp(amount: number): string {
		return `Rp ${amount.toLocaleString('id-ID')}`;
	}

	function statusVariant(status: string): 'default' | 'secondary' | 'destructive' | 'outline' {
		switch (status) {
			case 'success':
				return 'default';
			case 'failed':
				return 'destructive';
			case 'pending':
				return 'outline';
			default:
				return 'secondary';
		}
	}

	function handlePanelError(error: unknown, fallbackMessage: string) {
		if (error instanceof ApiError && error.status === 403) {
			isUnauthorized = true;
			errorMessage = '';
			return;
		}
		isUnauthorized = false;
		errorMessage = error instanceof Error ? error.message : fallbackMessage;
		toast.error(errorMessage);
	}
</script>

<div class="space-y-4">
	{#if canManage}
		<Card>
			<CardHeader>
				<CardTitle>New Payout</CardTitle>
				<CardDescription>
					Select a verified bank account and enter the amount to withdraw. Preview shows fees and net amount before confirmation.
				</CardDescription>
			</CardHeader>
			<CardContent class="space-y-4">
				{#if balance}
					<div class="rounded-lg border bg-muted/30 p-3 text-sm">
						<span class="text-muted-foreground">Withdrawable balance:</span>
						<span class="ml-2 font-medium">{formatRp(balance.withdrawable_balance)}</span>
					</div>
				{/if}

				<form class="grid gap-4 md:grid-cols-[1fr_1fr_auto]" onsubmit={handlePreview}>
					<div class="grid gap-2">
						<Label for="payout-bank">Bank account</Label>
						<Select
							type="single"
							value={selectedBankId}
							onValueChange={(v) => (selectedBankId = v)}
						>
							<SelectTrigger id="payout-bank">
								<span>
									{#if selectedBankId}
										{@const bank = banks.find((b) => b.id === selectedBankId)}
										{bank ? `${bank.bank_name} ••${bank.account_number_last4}` : 'Select bank'}
									{:else}
										Select bank
									{/if}
								</span>
							</SelectTrigger>
							<SelectContent>
								{#each banks as bank}
									<SelectItem value={bank.id}>
										{bank.bank_name} — {bank.account_holder_name} (••{bank.account_number_last4})
										{#if bank.is_default} ★{/if}
									</SelectItem>
								{/each}
							</SelectContent>
						</Select>
					</div>

					<div class="grid gap-2">
						<Label for="payout-amount">Amount (Rp)</Label>
						<Input
							id="payout-amount"
							type="number"
							min="1"
							bind:value={requestedAmount}
							placeholder="50000"
							disabled={isLoading || isPreviewLoading || isConfirming}
							required
						/>
					</div>

					<div class="flex items-end">
						<Button
							type="submit"
							class="w-full md:w-auto"
							disabled={isLoading || isPreviewLoading || isConfirming || !selectedBankId}
						>
							{#if isPreviewLoading}
								<Loader2 class="mr-2 h-4 w-4 animate-spin" />
								Previewing...
							{:else}
								<ArrowDownToLine class="mr-2 h-4 w-4" />
								Preview
							{/if}
						</Button>
					</div>
				</form>

				{#if preview}
					<div class="space-y-3 rounded-lg border border-blue-300 bg-blue-50 p-4 text-sm text-blue-950">
						<div class="flex flex-wrap items-center justify-between gap-3">
							<div>
								<p class="font-medium">Payout Preview</p>
								<p class="text-blue-900/80">
									Review the fee breakdown below. Confirm only if correct.
								</p>
							</div>
							<Button type="button" onclick={handleConfirm} disabled={isConfirming}>
								{#if isConfirming}
									<Loader2 class="mr-2 h-4 w-4 animate-spin" />
									Confirming...
								{:else}
									<Send class="mr-2 h-4 w-4" />
									Confirm Payout
								{/if}
							</Button>
						</div>

						<div class="grid gap-2 rounded-md border border-blue-200 bg-background p-3 md:grid-cols-2">
							<p><span class="font-medium">Bank:</span> {preview.bank_name}</p>
							<p><span class="font-medium">Account:</span> {preview.account_holder_name} (••{preview.account_number_last4})</p>
							<p><span class="font-medium">Requested:</span> {formatRp(preview.requested_amount)}</p>
							<p><span class="font-medium">Platform fee ({preview.platform_withdraw_fee_bps / 100}%):</span> {formatRp(preview.platform_withdraw_fee_amount)}</p>
							<p><span class="font-medium">Provider fee:</span> {formatRp(preview.provider_withdraw_fee_amount)}</p>
							<p><span class="font-medium text-emerald-700">Net disbursed:</span> <span class="font-semibold text-emerald-700">{formatRp(preview.net_disbursed_amount)}</span></p>
						</div>
					</div>
				{/if}
			</CardContent>
		</Card>
	{/if}

	<Card>
		<CardHeader>
			<div class="flex items-start justify-between gap-3">
				<div>
					<CardTitle>Payout History</CardTitle>
					<CardDescription>
						All payout requests from this store, with their current status.
					</CardDescription>
				</div>
				<Button
					type="button"
					variant="outline"
					size="sm"
					onclick={() => void loadData()}
					disabled={isLoading}
				>
					<RefreshCcw class="mr-2 h-4 w-4" />
					Refresh
				</Button>
			</div>
		</CardHeader>
		<CardContent class="space-y-4">
			{#if isLoading}
				<div class="flex justify-center p-10">
					<Loader2 class="h-6 w-6 animate-spin text-muted-foreground" />
				</div>
			{:else if isUnauthorized}
				<div class="rounded-lg border border-dashed p-8 text-sm text-muted-foreground">
					You do not have permission to view payouts for this store.
				</div>
			{:else if errorMessage}
				<div class="space-y-3 rounded-lg border border-dashed p-8 text-sm">
					<p class="text-destructive">{errorMessage}</p>
					<Button type="button" variant="outline" size="sm" onclick={() => void loadData()}>
						Try Again
					</Button>
				</div>
			{:else if payouts.length === 0}
				<div class="rounded-lg border border-dashed p-8 text-sm text-muted-foreground">
					No payout requests yet.
				</div>
			{:else}
				<div class="rounded-md border">
					<Table>
						<TableHeader>
							<TableRow>
								<TableHead>Date</TableHead>
								<TableHead>Destination</TableHead>
								<TableHead class="text-right">Amount</TableHead>
								<TableHead class="text-right">Net</TableHead>
								<TableHead>Status</TableHead>
							</TableRow>
						</TableHeader>
						<TableBody>
							{#each payouts as payout}
								<TableRow>
									<TableCell>
										<span class="text-sm">{new Date(payout.created_at).toLocaleDateString('id-ID')}</span>
									</TableCell>
									<TableCell>
										<div class="space-y-0.5">
											<p class="font-medium text-sm">{payout.bank_name}</p>
											<p class="text-xs text-muted-foreground">{payout.account_holder_name} ••{payout.account_number_last4}</p>
										</div>
									</TableCell>
									<TableCell class="text-right font-medium">
										{formatRp(payout.requested_amount)}
									</TableCell>
									<TableCell class="text-right text-sm">
										{formatRp(payout.net_disbursed_amount)}
									</TableCell>
									<TableCell>
										<Badge variant={statusVariant(payout.status)} class="capitalize">
											{payout.status}
										</Badge>
									</TableCell>
								</TableRow>
							{/each}
						</TableBody>
					</Table>
				</div>
			{/if}
		</CardContent>
	</Card>
</div>
