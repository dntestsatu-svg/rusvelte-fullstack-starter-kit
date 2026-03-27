<script module lang="ts">
	export interface SettlementStoreOption {
		id: string;
		name: string;
	}

	export interface SettlementBalancePreview {
		pending_balance: number;
		settled_balance: number;
		withdrawable_balance: number;
	}

	export interface SettlementReceipt {
		id: string;
		amount: number;
		storeName: string;
		pending_balance: number;
		settled_balance: number;
		withdrawable_balance: number;
		created_at: string;
	}
</script>

<script lang="ts">
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
	import { Textarea } from '$lib/components/ui/textarea';
	import { ArrowRightLeft, BadgeInfo, CircleAlert, Landmark, Wallet } from '@lucide/svelte';

	let {
		isDev = false,
		storeState = 'loading',
		storeOptions = [],
		selectedStoreId = '',
		activeStoreName = '',
		requiresSelection = false,
		storeErrorMessage = '',
		balanceState = 'idle',
		balance = null,
		balanceErrorMessage = '',
		amountValue = '',
		notesValue = '',
		submitState = 'idle',
		submitErrorMessage = '',
		receipt = null,
		onStoreChange = null,
		onAmountInput = null,
		onNotesInput = null,
		onSubmit = null
	} = $props<{
		isDev?: boolean;
		storeState?: 'loading' | 'ready' | 'empty' | 'error';
		storeOptions?: SettlementStoreOption[];
		selectedStoreId?: string;
		activeStoreName?: string;
		requiresSelection?: boolean;
		storeErrorMessage?: string;
		balanceState?: 'idle' | 'loading' | 'ready' | 'empty' | 'error';
		balance?: SettlementBalancePreview | null;
		balanceErrorMessage?: string;
		amountValue?: string;
		notesValue?: string;
		submitState?: 'idle' | 'submitting' | 'success' | 'error';
		submitErrorMessage?: string;
		receipt?: SettlementReceipt | null;
		onStoreChange?: ((value: string) => void) | null;
		onAmountInput?: ((value: string) => void) | null;
		onNotesInput?: ((value: string) => void) | null;
		onSubmit?: (() => void) | null;
	}>();

	const hasMultipleStores = $derived(storeOptions.length > 1);
	const canSubmit = $derived(
		isDev &&
			storeState === 'ready' &&
			balanceState === 'ready' &&
			!requiresSelection &&
			submitState !== 'submitting'
	);

	function formatMoney(amount: number): string {
		return new Intl.NumberFormat('id-ID').format(amount);
	}
</script>

<div class="space-y-6">
	<div class="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
		<div>
			<h1 class="text-3xl font-bold tracking-tight">Settlement Center</h1>
			<p class="text-muted-foreground">
				Process manual developer settlement from pending balance into settled balance.
			</p>
		</div>
		<div class="inline-flex items-center gap-2 rounded-full border bg-muted/40 px-4 py-2 text-sm text-muted-foreground">
			<Landmark class="h-4 w-4" />
			<span>Dev-only finance surface</span>
		</div>
	</div>

	{#if !isDev}
		<Card>
			<CardHeader>
				<CardTitle>Access restricted</CardTitle>
				<CardDescription>Settlement mutation is limited to developer accounts.</CardDescription>
			</CardHeader>
			<CardContent class="text-sm text-muted-foreground">
				Only `dev` can move balance from pending to settled. This page never authorizes finance mutation from the frontend alone.
			</CardContent>
		</Card>
	{:else}
		<Card class="border-primary/20 bg-primary/5">
			<CardHeader>
				<CardTitle class="flex items-center gap-2">
					<ArrowRightLeft class="h-5 w-5 text-primary" />
					Settlement Rule
				</CardTitle>
				<CardDescription>
					Settlement moves money from pending to settled. It does not create new funds.
				</CardDescription>
			</CardHeader>
			<CardContent class="grid gap-3 text-sm text-muted-foreground md:grid-cols-3">
				<p>Pending balance comes from successful payments after the 3% platform fee and cannot be withdrawn yet.</p>
				<p>Settled balance is manually processed by developer and becomes available for withdrawal flow later.</p>
				<p>Withdrawable stays derived from settled minus reserved balance. This page does not touch payout or reserve logic.</p>
			</CardContent>
		</Card>

		<Card>
			<CardHeader>
				<CardTitle>Settlement Scope</CardTitle>
				<CardDescription>Select the exact store whose balance will be moved.</CardDescription>
			</CardHeader>
			<CardContent class="space-y-4">
				{#if storeState === 'loading'}
					<p class="text-sm text-muted-foreground">Loading available stores...</p>
				{:else if storeState === 'error'}
					<div class="rounded-lg border border-destructive/30 bg-destructive/5 p-4 text-sm text-destructive">
						{storeErrorMessage || 'We could not load stores for settlement.'}
					</div>
				{:else if storeState === 'empty'}
					<p class="text-sm text-muted-foreground">No stores available for settlement yet.</p>
				{:else}
					<div class="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
						<div>
							<p class="text-sm font-medium">Store in scope</p>
							<p class="text-xs text-muted-foreground">
								{#if requiresSelection}
									Select a store before entering a settlement amount.
								{:else if activeStoreName}
									Active store: {activeStoreName}
								{:else}
									Select a store before entering a settlement amount.
								{/if}
							</p>
						</div>
						{#if hasMultipleStores}
							<Select
								type="single"
								value={selectedStoreId}
								onValueChange={(value) => {
									onStoreChange?.(value);
								}}
							>
								<SelectTrigger class="w-[260px]">
									<span>{requiresSelection ? 'Select a store' : activeStoreName}</span>
								</SelectTrigger>
								<SelectContent>
									{#each storeOptions as store}
										<SelectItem value={store.id}>{store.name}</SelectItem>
									{/each}
								</SelectContent>
							</Select>
						{:else if activeStoreName}
							<div class="rounded-lg border bg-muted/30 px-4 py-2 text-sm font-medium">
								{activeStoreName}
							</div>
						{/if}
					</div>
				{/if}
			</CardContent>
		</Card>

		<Card>
			<CardHeader>
				<CardTitle class="flex items-center gap-2">
					<Wallet class="h-5 w-5 text-primary" />
					Balance Before Processing
				</CardTitle>
				<CardDescription>
					Transparent context for the store balance you are about to move.
				</CardDescription>
			</CardHeader>
			<CardContent>
				{#if balanceState === 'loading'}
					<p class="text-sm text-muted-foreground">Loading current balance snapshot...</p>
				{:else if balanceState === 'error'}
					<div class="rounded-lg border border-destructive/30 bg-destructive/5 p-4 text-sm text-destructive">
						{balanceErrorMessage || 'Current store balance could not be loaded.'}
					</div>
				{:else if balanceState === 'idle' || balanceState === 'empty'}
					<p class="text-sm text-muted-foreground">Choose a store to preview its current balances.</p>
				{:else if balance}
					<div class="grid gap-4 md:grid-cols-3">
						<div class="rounded-xl border bg-muted/30 p-4">
							<div class="flex items-center justify-between">
								<p class="text-sm text-muted-foreground">Pending Balance</p>
								<BadgeInfo class="h-4 w-4 text-muted-foreground" title="Pending balance = hasil payment sukses setelah potong fee 3%, belum bisa ditarik." />
							</div>
							<p class="mt-2 text-2xl font-semibold">Rp {formatMoney(balance.pending_balance)}</p>
						</div>
						<div class="rounded-xl border bg-muted/30 p-4">
							<div class="flex items-center justify-between">
								<p class="text-sm text-muted-foreground">Settled Balance</p>
								<BadgeInfo class="h-4 w-4 text-muted-foreground" title="Settled balance = saldo yang sudah disettle manual oleh developer." />
							</div>
							<p class="mt-2 text-2xl font-semibold">Rp {formatMoney(balance.settled_balance)}</p>
						</div>
						<div class="rounded-xl border bg-muted/30 p-4">
							<div class="flex items-center justify-between">
								<p class="text-sm text-muted-foreground">Withdrawable Balance</p>
								<BadgeInfo class="h-4 w-4 text-muted-foreground" title="Withdrawable balance = settled - reserved." />
							</div>
							<p class="mt-2 text-2xl font-semibold">Rp {formatMoney(balance.withdrawable_balance)}</p>
						</div>
					</div>
				{/if}
			</CardContent>
		</Card>

		<Card>
			<CardHeader>
				<CardTitle>Process Settlement</CardTitle>
				<CardDescription>
					Enter the amount to move from pending into settled for the selected store.
				</CardDescription>
			</CardHeader>
			<CardContent class="space-y-4">
				<form
					class="space-y-4"
					onsubmit={(event) => {
						event.preventDefault();
						onSubmit?.();
					}}
				>
					<div class="grid gap-2">
						<Label for="settlement-amount">Settlement amount</Label>
						<Input
							id="settlement-amount"
							type="number"
							min="1"
							step="1"
							inputmode="numeric"
							placeholder="30000"
							value={amountValue}
							oninput={(event) => {
								onAmountInput?.((event.currentTarget as HTMLInputElement).value);
							}}
						/>
						<p class="text-xs text-muted-foreground">
							Use integer Rupiah only. Backend still validates amount &gt; 0 and amount &lt;= current pending balance.
						</p>
					</div>

					<div class="grid gap-2">
						<Label for="settlement-notes">Notes</Label>
						<Textarea
							id="settlement-notes"
							rows={4}
							placeholder="Optional internal note for the settlement record"
							value={notesValue}
							oninput={(event) => {
								onNotesInput?.((event.currentTarget as HTMLTextAreaElement).value);
							}}
						/>
					</div>

					<Button type="submit" class="w-full md:w-auto" disabled={!canSubmit}>
						{submitState === 'submitting' ? 'Processing settlement...' : 'Process settlement'}
					</Button>
				</form>

				{#if submitState === 'error'}
					<div class="rounded-lg border border-destructive/30 bg-destructive/5 p-4 text-sm text-destructive">
						<div class="flex items-start gap-2">
							<CircleAlert class="mt-0.5 h-4 w-4 shrink-0" />
							<p>{submitErrorMessage || 'Settlement could not be processed.'}</p>
						</div>
					</div>
				{/if}

				{#if submitState === 'success' && receipt}
					<div class="rounded-lg border border-emerald-500/30 bg-emerald-500/5 p-4 text-sm text-emerald-700 dark:text-emerald-300">
						<p class="font-semibold">Settlement processed</p>
						<p class="mt-1">
							{receipt.storeName} moved Rp {formatMoney(receipt.amount)} from pending to settled.
						</p>
						<p class="mt-1">
							Pending now Rp {formatMoney(receipt.pending_balance)}, settled now Rp {formatMoney(receipt.settled_balance)}, withdrawable now Rp {formatMoney(receipt.withdrawable_balance)}.
						</p>
					</div>
				{/if}
			</CardContent>
		</Card>
	{/if}
</div>
