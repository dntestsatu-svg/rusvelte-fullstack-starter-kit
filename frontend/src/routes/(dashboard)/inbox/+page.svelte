<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { apiFetch } from '$lib/api/client';
	import * as Table from '$lib/components/ui/table';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { toast } from 'svelte-sonner';
	import { formatDistanceToNow } from 'date-fns';
	import { MessageSquare, Clock, ShieldAlert, User, Search } from '@lucide/svelte';
	import { Input } from '$lib/components/ui/input';

	let threads = $state<any[]>([]);
	let pagination = $state({ limit: 20, offset: 0 });
	let isLoading = $state(true);
	let searchQuery = $state('');
	let loadError = $state('');
	const canAccessInbox = $derived(['dev', 'superadmin'].includes(page.data.sessionUser?.role ?? ''));

	onMount(() => {
		if (!canAccessInbox) {
			isLoading = false;
			return;
		}
		void loadThreads();
	});

	async function loadThreads() {
		isLoading = true;
		loadError = '';
		try {
			const json = await apiFetch<{ data: any[]; pagination: { limit?: number; offset?: number; page?: number; per_page?: number } }>(
				`/api/v1/support/inbox?limit=${pagination.limit}&offset=${pagination.offset}`
			);
			threads = json.data;
			pagination = { ...pagination, ...json.pagination };
		} catch (err) {
			loadError = err instanceof Error ? err.message : 'An error occurred while loading threads';
			toast.error(loadError);
		} finally {
			isLoading = false;
		}
	}

	function getStatusVariant(status: string) {
		switch (status) {
			case 'open': return 'default';
			case 'in_progress': return 'secondary';
			case 'closed': return 'outline';
			case 'spam': return 'destructive';
			default: return 'outline';
		}
	}
	const filteredThreads = $derived(
		threads.filter(t => 
			t.subject.toLowerCase().includes(searchQuery.toLowerCase()) ||
			t.email.toLowerCase().includes(searchQuery.toLowerCase()) ||
			t.name.toLowerCase().includes(searchQuery.toLowerCase())
		)
	);
</script>

<div class="space-y-6">
	<div class="flex flex-col md:flex-row justify-between items-start md:items-center gap-4">
		<div>
			<h1 class="text-3xl font-bold tracking-tight">Support Inbox</h1>
			<p class="text-muted-foreground">Manage incoming support inquiries and customer threads.</p>
		</div>
		<div class="relative w-full md:w-64">
			<Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
			<Input
				type="search"
				placeholder="Search threads..."
				class="pl-9"
				bind:value={searchQuery}
			/>
		</div>
	</div>

	<div class="border rounded-xl bg-card shadow-sm overflow-hidden">
		{#if !canAccessInbox}
			<div class="h-32 flex items-center justify-center text-sm text-muted-foreground">
				Access to the support inbox is restricted to dev and superadmin users.
			</div>
		{:else}
		<Table.Root>
			<Table.Header>
				<Table.Row>
					<Table.TableHead class="w-[300px]">Customer / Subject</Table.TableHead>
					<Table.TableHead>Status</Table.TableHead>
					<Table.TableHead>Category</Table.TableHead>
					<Table.TableHead>Last Activity</Table.TableHead>
					<Table.TableHead class="text-right">Action</Table.TableHead>
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#if isLoading}
					{#each Array(5) as _}
						<Table.Row>
							<Table.TableCell><div class="h-4 w-48 bg-muted animate-pulse rounded"></div></Table.TableCell>
							<Table.TableCell><div class="h-4 w-16 bg-muted animate-pulse rounded"></div></Table.TableCell>
							<Table.TableCell><div class="h-4 w-20 bg-muted animate-pulse rounded"></div></Table.TableCell>
							<Table.TableCell><div class="h-4 w-24 bg-muted animate-pulse rounded"></div></Table.TableCell>
							<Table.TableCell class="text-right"><div class="h-8 w-16 bg-muted animate-pulse rounded ml-auto"></div></Table.TableCell>
						</Table.Row>
					{/each}
				{:else if loadError}
					<Table.Row>
						<Table.TableCell colspan={5} class="h-32 text-center">
							<div class="space-y-3">
								<p class="text-destructive">{loadError}</p>
								<Button variant="outline" size="sm" onclick={() => void loadThreads()}>
									Try Again
								</Button>
							</div>
						</Table.TableCell>
					</Table.Row>
				{:else if filteredThreads.length === 0}
					<Table.Row>
						<Table.TableCell colspan={5} class="h-32 text-center text-muted-foreground">
							No support threads found.
						</Table.TableCell>
					</Table.Row>
				{:else}
					{#each filteredThreads as thread}
						<Table.Row class="group hover:bg-muted/50 transition-colors">
							<Table.TableCell>
								<div class="flex flex-col">
									<span class="font-semibold text-foreground group-hover:text-primary transition-colors">
										{thread.subject}
									</span>
									<span class="text-xs text-muted-foreground flex items-center gap-1 mt-0.5">
										<User size={12} /> {thread.name} ({thread.email})
									</span>
								</div>
							</Table.TableCell>
							<Table.TableCell>
								<Badge variant={getStatusVariant(thread.status)} class="capitalize">
									{thread.status.replace('_', ' ')}
								</Badge>
							</Table.TableCell>
							<Table.TableCell>
								<Badge variant="outline" class="capitalize">
									{thread.category}
								</Badge>
							</Table.TableCell>
							<Table.TableCell>
								<div class="flex items-center gap-1.5 text-sm text-muted-foreground">
									<Clock size={14} />
									{formatDistanceToNow(new Date(thread.last_message_at || thread.created_at), { addSuffix: true })}
								</div>
							</Table.TableCell>
							<Table.TableCell class="text-right">
								<Button variant="ghost" size="sm" href="/dashboard/inbox/{thread.id}">
									View
								</Button>
							</Table.TableCell>
						</Table.Row>
					{/each}
				{/if}
			</Table.Body>
		</Table.Root>
		{/if}
	</div>
</div>
