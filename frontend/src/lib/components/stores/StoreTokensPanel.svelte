<script lang="ts">
	import { onMount } from 'svelte';

	import { ApiError } from '$lib/api/client';
	import { storesApi, type StoreApiToken } from '$lib/api/stores';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { formatStoreTokenTimestamp } from '$lib/stores/token-access';
	import { Copy, KeyRound, Loader2, RefreshCcw, Trash2 } from '@lucide/svelte';
	import { toast } from 'svelte-sonner';

	let { storeId }: { storeId: string } = $props();

	let tokens = $state<StoreApiToken[]>([]);
	let tokenName = $state('');
	let latestCreatedToken = $state<{ plaintext_token: string; token: StoreApiToken } | null>(null);
	let isLoading = $state(true);
	let isSubmitting = $state(false);
	let isRevoking = $state(false);
	let errorMessage = $state('');
	let isUnauthorized = $state(false);
	let confirmOpen = $state(false);
	let tokenPendingRevoke = $state<StoreApiToken | null>(null);

	onMount(() => {
		void loadTokens();
	});

	async function loadTokens() {
		isLoading = true;
		errorMessage = '';

		try {
			const response = await storesApi.listTokens(storeId);
			tokens = response.tokens;
			isUnauthorized = false;
		} catch (error) {
			handlePanelError(error, 'Failed to load store tokens');
		} finally {
			isLoading = false;
		}
	}

	async function handleCreateToken(event: Event) {
		event.preventDefault();

		const name = tokenName.trim();
		if (!name) {
			return;
		}

		isSubmitting = true;

		try {
			const response = await storesApi.createToken(storeId, { name });
			latestCreatedToken = response;
			tokenName = '';
			tokens = [response.token, ...tokens.filter((token) => token.id !== response.token.id)];
			isUnauthorized = false;
			errorMessage = '';
			toast.success('Store API token created');
		} catch (error) {
			handlePanelError(error, 'Failed to create store token');
		} finally {
			isSubmitting = false;
		}
	}

	function requestRevoke(token: StoreApiToken) {
		tokenPendingRevoke = token;
		confirmOpen = true;
	}

	async function confirmRevoke() {
		if (!tokenPendingRevoke) {
			return;
		}

		confirmOpen = false;
		isRevoking = true;

		try {
			await storesApi.revokeToken(storeId, tokenPendingRevoke.id);
			tokens = tokens.filter((token) => token.id !== tokenPendingRevoke?.id);
			if (latestCreatedToken?.token.id === tokenPendingRevoke.id) {
				latestCreatedToken = null;
			}
			toast.success('Store API token revoked');
		} catch (error) {
			handlePanelError(error, 'Failed to revoke store token');
		} finally {
			isRevoking = false;
			tokenPendingRevoke = null;
		}
	}

	function cancelRevoke() {
		confirmOpen = false;
		tokenPendingRevoke = null;
	}

	async function copyPlaintextToken() {
		if (!latestCreatedToken) {
			return;
		}

		try {
			await navigator.clipboard.writeText(latestCreatedToken.plaintext_token);
			toast.success('Token copied to clipboard');
		} catch {
			toast.error('Clipboard copy failed. Copy the token manually.');
		}
	}

	function dismissPlaintextToken() {
		latestCreatedToken = null;
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
	<form class="space-y-3 rounded-lg border bg-muted/30 p-4" onsubmit={handleCreateToken}>
		<div class="grid gap-2">
			<Label for="token-name">Token name</Label>
			<Input
				id="token-name"
				bind:value={tokenName}
				placeholder="Production Store Client"
				disabled={isLoading || isUnauthorized || isSubmitting || isRevoking}
				required
			/>
		</div>

		<div class="flex flex-wrap items-center justify-between gap-3">
			<p class="text-sm text-muted-foreground">
				Create an active bearer token for Store Client API access. Plaintext is shown only once.
			</p>
			<Button type="submit" disabled={isLoading || isUnauthorized || isSubmitting || isRevoking}>
				{#if isSubmitting}
					<Loader2 class="mr-2 h-4 w-4 animate-spin" />
					Creating...
				{:else}
					<KeyRound class="mr-2 h-4 w-4" />
					Create Token
				{/if}
			</Button>
		</div>
	</form>

	{#if latestCreatedToken}
		<div class="space-y-3 rounded-lg border border-amber-300 bg-amber-50 p-4 text-sm text-amber-950">
			<div class="flex flex-wrap items-start justify-between gap-3">
				<div class="space-y-1">
					<p class="font-medium">Copy this token now</p>
					<p class="text-amber-900/80">
						This plaintext token will not be shown again after you dismiss or reload this page.
					</p>
				</div>
				<div class="flex gap-2">
					<Button type="button" variant="outline" size="sm" onclick={copyPlaintextToken}>
						<Copy class="mr-2 h-4 w-4" />
						Copy
					</Button>
					<Button type="button" variant="outline" size="sm" onclick={dismissPlaintextToken}>
						Dismiss
					</Button>
				</div>
			</div>

			<div class="rounded-md border border-amber-200 bg-background p-3">
				<p class="mb-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
					{latestCreatedToken.token.name}
				</p>
				<code class="block break-all font-mono text-xs">{latestCreatedToken.plaintext_token}</code>
			</div>
		</div>
	{/if}

	<div class="rounded-lg border">
		<div class="flex items-center justify-between border-b px-4 py-3">
			<div>
				<h3 class="font-semibold">Active Tokens</h3>
				<p class="text-sm text-muted-foreground">
					Only active tokens are listed here. Revocation history remains in audit logs.
				</p>
			</div>
			<Button
				type="button"
				variant="outline"
				size="sm"
				onclick={() => void loadTokens()}
				disabled={isLoading || isSubmitting || isRevoking}
			>
				<RefreshCcw class="mr-2 h-4 w-4" />
				Refresh
			</Button>
		</div>

		{#if isLoading}
			<div class="flex justify-center p-10">
				<Loader2 class="h-6 w-6 animate-spin text-muted-foreground" />
			</div>
		{:else if isUnauthorized}
			<div class="p-6 text-sm text-muted-foreground">
				You do not have permission to view or manage store API tokens for this store.
			</div>
		{:else if errorMessage}
			<div class="space-y-3 p-6 text-sm">
				<p class="text-destructive">{errorMessage}</p>
				<Button type="button" variant="outline" size="sm" onclick={() => void loadTokens()}>
					Try Again
				</Button>
			</div>
		{:else if tokens.length === 0}
			<div class="p-6 text-sm text-muted-foreground">
				No active store API tokens yet.
			</div>
		{:else}
			<div class="divide-y">
				{#each tokens as token}
					<div class="flex flex-col gap-4 p-4 md:flex-row md:items-center md:justify-between">
						<div class="space-y-2">
							<div class="flex flex-wrap items-center gap-2">
								<p class="font-medium">{token.name}</p>
								<Badge variant="outline" class="font-mono">
									{token.display_prefix}
								</Badge>
							</div>
							<div class="grid gap-1 text-sm text-muted-foreground">
								<p>Created: {formatStoreTokenTimestamp(token.created_at)}</p>
								<p>Last used: {formatStoreTokenTimestamp(token.last_used_at)}</p>
								{#if token.expires_at}
									<p>Expires: {formatStoreTokenTimestamp(token.expires_at)}</p>
								{/if}
							</div>
						</div>

						<div class="flex justify-end">
							<Button
								type="button"
								variant="destructive"
								size="sm"
								onclick={() => requestRevoke(token)}
								disabled={isSubmitting || isRevoking}
							>
								{#if isRevoking && tokenPendingRevoke?.id === token.id}
									<Loader2 class="mr-2 h-4 w-4 animate-spin" />
									Revoking...
								{:else}
									<Trash2 class="mr-2 h-4 w-4" />
									Revoke
								{/if}
							</Button>
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>

<ConfirmDialog
	bind:open={confirmOpen}
	title="Revoke store token?"
	description={`This will immediately disable ${tokenPendingRevoke?.name ?? 'this token'} for Store Client API access.`}
	onConfirm={() => void confirmRevoke()}
	onCancel={cancelRevoke}
/>
