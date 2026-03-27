<script lang="ts">
	import { onMount } from 'svelte';

	import { ApiError } from '$lib/api/client';
	import { storesApi, type StoreBankAccount, type StoreBankInquiry } from '$lib/api/stores';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '$lib/components/ui/card';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import {
		applyDefaultBankRecord,
		formatMaskedBankAccount,
		mergeSavedBankRecord,
		shouldDefaultNewBank
	} from '$lib/stores/bank-access';
	import {
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow
	} from '$lib/components/ui/table';
	import { BadgeCheck, Landmark, Loader2, RefreshCcw, ShieldCheck } from '@lucide/svelte';
	import { toast } from 'svelte-sonner';

	let {
		storeId,
		canManage
	}: {
		storeId: string;
		canManage: boolean;
	} = $props();

	let banks = $state<StoreBankAccount[]>([]);
	let bankCode = $state('');
	let accountNumber = $state('');
	let markAsDefault = $state(true);
	let latestInquiry = $state<StoreBankInquiry | null>(null);
	let isLoading = $state(true);
	let isInquiryLoading = $state(false);
	let isSaving = $state(false);
	let defaultBankPending = $state<string | null>(null);
	let errorMessage = $state('');
	let isUnauthorized = $state(false);

	onMount(() => {
		void loadBanks();
	});

	async function loadBanks() {
		isLoading = true;
		errorMessage = '';

		try {
			const response = await storesApi.listBanks(storeId);
			banks = response.banks;
			markAsDefault = shouldDefaultNewBank(response.banks.length);
			isUnauthorized = false;
		} catch (error) {
			handlePanelError(error, 'Failed to load bank accounts');
		} finally {
			isLoading = false;
		}
	}

	async function handleInquiry(event: Event) {
		event.preventDefault();
		if (!canManage) {
			return;
		}

		isInquiryLoading = true;

		try {
			const response = await storesApi.inquireBank(storeId, {
				bank_code: bankCode.trim(),
				account_number: accountNumber.trim()
			});
			latestInquiry = response.inquiry;
			errorMessage = '';
			isUnauthorized = false;
			toast.success('Bank account verified');
		} catch (error) {
			latestInquiry = null;
			handlePanelError(error, 'Failed to verify bank account');
		} finally {
			isInquiryLoading = false;
		}
	}

	async function handleSaveBank() {
		if (!canManage || !latestInquiry) {
			return;
		}

		isSaving = true;

		try {
			const response = await storesApi.createBank(storeId, {
				bank_code: bankCode.trim(),
				account_number: accountNumber.trim(),
				is_default: markAsDefault
			});
			banks = mergeSavedBankRecord(banks, response.bank);
			latestInquiry = null;
			accountNumber = '';
			bankCode = '';
			markAsDefault = shouldDefaultNewBank(banks.length);
			isUnauthorized = false;
			errorMessage = '';
			toast.success('Verified bank account saved');
		} catch (error) {
			handlePanelError(error, 'Failed to save bank account');
		} finally {
			isSaving = false;
		}
	}

	async function handleSetDefault(bank: StoreBankAccount) {
		if (!canManage || bank.is_default) {
			return;
		}

		defaultBankPending = bank.id;
		try {
			const response = await storesApi.setDefaultBank(storeId, bank.id);
			banks = applyDefaultBankRecord(banks, response.bank.id);
			toast.success(`${bank.bank_name} ending ${bank.account_number_last4} is now the default bank`);
		} catch (error) {
			handlePanelError(error, 'Failed to update the default bank');
		} finally {
			defaultBankPending = null;
		}
	}

	function toggleDefaultPreference() {
		if (!canManage) {
			return;
		}

		markAsDefault = !markAsDefault;
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
				<CardTitle>Add Payout Bank Account</CardTitle>
				<CardDescription>
					Verify the account first. Only verified bank accounts can be saved and only masked last4 is shown after that.
				</CardDescription>
			</CardHeader>
			<CardContent class="space-y-4">
				<form class="grid gap-4 md:grid-cols-[1fr_1fr_auto]" onsubmit={handleInquiry}>
					<div class="grid gap-2">
						<Label for="bank-code">Bank code</Label>
						<Input
							id="bank-code"
							bind:value={bankCode}
							placeholder="014"
							disabled={isLoading || isInquiryLoading || isSaving}
							required
						/>
					</div>

					<div class="grid gap-2">
						<Label for="account-number">Account number</Label>
						<Input
							id="account-number"
							bind:value={accountNumber}
							inputmode="numeric"
							placeholder="1234567890"
							disabled={isLoading || isInquiryLoading || isSaving}
							required
						/>
					</div>

					<div class="flex items-end">
						<Button type="submit" class="w-full md:w-auto" disabled={isLoading || isInquiryLoading || isSaving}>
							{#if isInquiryLoading}
								<Loader2 class="mr-2 h-4 w-4 animate-spin" />
								Verifying...
							{:else}
								<ShieldCheck class="mr-2 h-4 w-4" />
								Verify
							{/if}
						</Button>
					</div>
				</form>

				<div class="flex flex-wrap items-center justify-between gap-3 rounded-lg border bg-muted/30 p-3 text-sm">
					<p class="text-muted-foreground">
						Default bank is used first for payout flow. Only one default bank can exist per store.
					</p>
					<Button
						type="button"
						variant={markAsDefault ? 'default' : 'outline'}
						size="sm"
						onclick={toggleDefaultPreference}
						disabled={isLoading || isInquiryLoading || isSaving}
					>
						{markAsDefault ? 'Will save as default' : 'Save without changing default'}
					</Button>
				</div>

				{#if latestInquiry}
					<div class="space-y-3 rounded-lg border border-emerald-300 bg-emerald-50 p-4 text-sm text-emerald-950">
						<div class="flex flex-wrap items-center justify-between gap-3">
							<div>
								<p class="font-medium">Verified account</p>
								<p class="text-emerald-900/80">
									Provider inquiry confirmed this payout destination. Save only if it matches the store owner intent.
								</p>
							</div>
							<Button type="button" onclick={handleSaveBank} disabled={isSaving}>
								{#if isSaving}
									<Loader2 class="mr-2 h-4 w-4 animate-spin" />
									Saving...
								{:else}
									<BadgeCheck class="mr-2 h-4 w-4" />
									Save verified bank
								{/if}
							</Button>
						</div>

						<div class="grid gap-2 rounded-md border border-emerald-200 bg-background p-3 md:grid-cols-2">
							<p><span class="font-medium">Bank:</span> {latestInquiry.bank_name} ({latestInquiry.bank_code})</p>
							<p><span class="font-medium">Account holder:</span> {latestInquiry.account_holder_name}</p>
							<p><span class="font-medium">Masked account:</span> {formatMaskedBankAccount(latestInquiry.account_number_last4)}</p>
							<p><span class="font-medium">Provider fee:</span> Rp {latestInquiry.provider_fee_amount.toLocaleString('id-ID')}</p>
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
					<CardTitle>Saved Bank Accounts</CardTitle>
					<CardDescription>
						UI only shows masked last4. Full account numbers stay encrypted at rest and are never rendered here.
					</CardDescription>
				</div>
				<Button type="button" variant="outline" size="sm" onclick={() => void loadBanks()} disabled={isLoading || isInquiryLoading || isSaving}>
					<RefreshCcw class="mr-2 h-4 w-4" />
					Refresh
				</Button>
			</div>
		</CardHeader>
		<CardContent class="space-y-4">
			{#if !canManage && !isUnauthorized}
				<div class="rounded-lg border border-dashed p-4 text-sm text-muted-foreground">
					Read-only mode. You can inspect masked payout bank metadata, but only store owners and dev can manage it.
				</div>
			{/if}

			{#if isLoading}
				<div class="flex justify-center p-10">
					<Loader2 class="h-6 w-6 animate-spin text-muted-foreground" />
				</div>
			{:else if isUnauthorized}
				<div class="rounded-lg border border-dashed p-8 text-sm text-muted-foreground">
					You do not have permission to view bank accounts for this store.
				</div>
			{:else if errorMessage}
				<div class="space-y-3 rounded-lg border border-dashed p-8 text-sm">
					<p class="text-destructive">{errorMessage}</p>
					<Button type="button" variant="outline" size="sm" onclick={() => void loadBanks()}>
						Try Again
					</Button>
				</div>
			{:else if banks.length === 0}
				<div class="rounded-lg border border-dashed p-8 text-sm text-muted-foreground">
					No verified bank accounts yet.
				</div>
			{:else}
				<div class="rounded-md border">
					<Table>
						<TableHeader>
							<TableRow>
								<TableHead>Bank</TableHead>
								<TableHead>Account</TableHead>
								<TableHead>Status</TableHead>
								<TableHead class="text-right">Action</TableHead>
							</TableRow>
						</TableHeader>
						<TableBody>
							{#each banks as bank}
								<TableRow>
									<TableCell>
										<div class="space-y-1">
											<p class="font-medium">{bank.bank_name}</p>
											<p class="text-sm text-muted-foreground">Code {bank.bank_code}</p>
										</div>
									</TableCell>
									<TableCell>
										<div class="space-y-1">
											<p class="font-medium">{bank.account_holder_name}</p>
											<p class="text-sm text-muted-foreground">{formatMaskedBankAccount(bank.account_number_last4)}</p>
										</div>
									</TableCell>
									<TableCell>
										<div class="flex flex-wrap items-center gap-2">
											<Badge variant="outline" class="capitalize">{bank.verification_status}</Badge>
											{#if bank.is_default}
												<Badge>
													Default
												</Badge>
											{/if}
										</div>
									</TableCell>
									<TableCell class="text-right">
										{#if canManage}
											<Button
												type="button"
												variant={bank.is_default ? 'secondary' : 'outline'}
												size="sm"
												disabled={bank.is_default || defaultBankPending === bank.id}
												onclick={() => void handleSetDefault(bank)}
											>
												{#if defaultBankPending === bank.id}
													<Loader2 class="mr-2 h-4 w-4 animate-spin" />
													Updating...
												{:else if bank.is_default}
													Default bank
												{:else}
													Set default
												{/if}
											</Button>
										{:else}
											<span class="text-sm text-muted-foreground">Read-only</span>
										{/if}
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
