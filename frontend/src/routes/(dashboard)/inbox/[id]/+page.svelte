<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { apiFetch } from '$lib/api/client';
	import { Button } from '$lib/components/ui/button';
	import { Textarea } from '$lib/components/ui/textarea';
	import { toast } from 'svelte-sonner';
	import { Badge } from '$lib/components/ui/badge';
	import * as Card from '$lib/components/ui/card';
	import * as Select from '$lib/components/ui/select';
	import { formatDistanceToNow, format } from 'date-fns';
	import { User, MessageSquare, ChevronLeft, Send, Clock, Shield } from '@lucide/svelte';

	let threadDetail = $state<any>(null);
	let replyBody = $state('');
	let isSubmitting = $state(false);
	let isLoading = $state(true);
	let loadError = $state('');

	const id = page.params.id;
	const canAccessInbox = $derived(['dev', 'superadmin'].includes(page.data.sessionUser?.role ?? ''));

	async function loadDetail() {
		isLoading = true;
		loadError = '';
		try {
			threadDetail = await apiFetch(`/api/v1/support/inbox/${id}`);
		} catch (err) {
			loadError = err instanceof Error ? err.message : 'An error occurred while loading details';
			toast.error(loadError);
		} finally {
			isLoading = false;
		}
	}

	async function handleReply() {
		if (!replyBody.trim() || isSubmitting) return;

		isSubmitting = true;
		try {
			await apiFetch(`/api/v1/support/inbox/${id}/reply`, {
				method: 'POST',
				body: JSON.stringify({ body: replyBody })
			});
			toast.success('Reply sent successfully');
			replyBody = '';
			await loadDetail();
		} catch (err) {
			toast.error('An error occurred while sending reply');
		} finally {
			isSubmitting = false;
		}
	}

	async function handleStatusChange(newStatus: string) {
		try {
			await apiFetch(`/api/v1/support/inbox/${id}/status`, {
				method: 'PATCH',
				body: JSON.stringify({ status: newStatus })
			});
			toast.success('Status updated');
			await loadDetail();
		} catch (err) {
			toast.error('An error occurred while updating status');
		}
	}

	onMount(() => {
		if (!canAccessInbox) {
			isLoading = false;
			return;
		}

		void loadDetail();
	});

	const statuses = [
		{ value: 'open', label: 'Open' },
		{ value: 'in_progress', label: 'In Progress' },
		{ value: 'closed', label: 'Closed' },
		{ value: 'spam', label: 'Spam' }
	];
</script>

<div class="space-y-6 pb-12">
	<div class="flex items-center gap-4">
		<Button variant="ghost" size="icon" href="/dashboard/inbox">
			<ChevronLeft class="h-5 w-5" />
		</Button>
		<div class="flex-1">
			<h1 class="text-2xl font-bold tracking-tight">Support Thread</h1>
			<p class="text-sm text-muted-foreground">Detailed view of the conversation history.</p>
		</div>
		{#if threadDetail}
			<div class="flex items-center gap-3">
				<Badge variant="outline" class="h-9 px-3 capitalize">
					{threadDetail.thread.status.replace('_', ' ')}
				</Badge>
				<Select.Root
					type="single"
					onValueChange={(value: string) => handleStatusChange(value)}
					value={threadDetail.thread.status}
				>
					<Select.Trigger class="w-40 h-9">
						<span>{statuses.find((item) => item.value === threadDetail.thread.status)?.label ?? 'Update Status'}</span>
					</Select.Trigger>
					<Select.Content>
						{#each statuses as s}
							<Select.Item value={s.value}>{s.label}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>
		{/if}
	</div>

	{#if isLoading}
		<div class="space-y-4">
			<div class="h-32 w-full bg-muted animate-pulse rounded-xl"></div>
			<div class="h-64 w-full bg-muted animate-pulse rounded-xl"></div>
		</div>
	{:else if !canAccessInbox}
		<div class="text-center py-20 border rounded-xl bg-card">
			<Shield class="h-12 w-12 mx-auto text-muted-foreground mb-4" />
			<h3 class="text-lg font-semibold">Access Denied</h3>
			<p class="text-muted-foreground">Only dev and superadmin users can access the support inbox.</p>
		</div>
	{:else if loadError}
		<div class="text-center py-20 border rounded-xl bg-card space-y-4">
			<Shield class="h-12 w-12 mx-auto text-muted-foreground" />
			<div class="space-y-2">
				<h3 class="text-lg font-semibold">Unable to load thread</h3>
				<p class="text-muted-foreground">{loadError}</p>
			</div>
			<Button variant="outline" onclick={() => void loadDetail()}>
				Try Again
			</Button>
		</div>
	{:else if !threadDetail}
		<div class="text-center py-20 border rounded-xl bg-card">
			<Shield class="h-12 w-12 mx-auto text-muted-foreground mb-4" />
			<h3 class="text-lg font-semibold">Thread Not Found</h3>
			<p class="text-muted-foreground">This support thread may have been deleted or is unavailable.</p>
		</div>
	{:else}
		<div class="grid grid-cols-1 lg:grid-cols-3 gap-8">
			<!-- Message History -->
			<div class="lg:col-span-2 space-y-6">
				<Card.Root>
					<Card.Header class="pb-3 border-b bg-muted/20">
						<div class="flex justify-between items-center">
							<Card.Title class="text-lg flex items-center gap-2">
								<MessageSquare class="h-5 w-5 text-primary" />
								{threadDetail.thread.subject}
							</Card.Title>
							<div class="text-xs text-muted-foreground flex items-center gap-1">
								<Clock class="h-3 w-3" />
								Started {format(new Date(threadDetail.thread.created_at), 'PPP')}
							</div>
						</div>
					</Card.Header>
					<Card.Content class="p-6 space-y-6">
						{#each threadDetail.messages as msg}
							<div class="flex gap-4 {msg.sender_type === 'staff' ? 'flex-row-reverse text-right' : ''}">
								<div class="w-10 h-10 rounded-full flex items-center justify-center shrink-0 {msg.sender_type === 'staff' ? 'bg-primary text-primary-foreground' : 'bg-muted text-muted-foreground'}">
									{#if msg.sender_type === 'staff'}
										<Shield class="h-5 w-5" />
									{:else}
										<User class="h-5 w-5" />
									{/if}
								</div>
								<div class="max-w-[80%] space-y-1">
									<div class="flex items-center gap-2 {msg.sender_type === 'staff' ? 'justify-end' : ''}">
										<span class="text-sm font-bold">
											{msg.sender_type === 'staff' ? 'Support Specialist' : threadDetail.thread.name}
										</span>
										<span class="text-[10px] text-muted-foreground">
											{formatDistanceToNow(new Date(msg.created_at), { addSuffix: true })}
										</span>
									</div>
									<div class="p-4 rounded-2xl text-sm leading-relaxed {msg.sender_type === 'staff' ? 'bg-primary text-primary-foreground rounded-tr-none' : 'bg-muted rounded-tl-none'}">
										{msg.body}
									</div>
								</div>
							</div>
						{/each}
					</Card.Content>
				</Card.Root>

				<!-- Reply Box -->
				<div class="space-y-4">
					<Textarea 
						placeholder="Draft your reply..." 
						bind:value={replyBody} 
						class="min-h-[150px] rounded-xl shadow-sm"
					/>
					<div class="flex justify-end">
						<Button onclick={() => handleReply()} disabled={isSubmitting || !replyBody.trim()}>
							<Send class="h-4 w-4 mr-2" />
							{isSubmitting ? 'Sending...' : 'Send Reply'}
						</Button>
					</div>
				</div>
			</div>

			<!-- Customer Info Sidebar -->
			<div class="space-y-6">
				<Card.Root>
					<Card.Header>
						<Card.Title class="text-lg">Customer Profile</Card.Title>
					</Card.Header>
					<Card.Content class="space-y-4">
						<div class="space-y-1">
							<span class="text-xs font-semibold text-muted-foreground uppercase tracking-wider">Full Name</span>
							<p class="text-sm font-medium">{threadDetail.thread.name}</p>
						</div>
						<div class="space-y-1">
							<span class="text-xs font-semibold text-muted-foreground uppercase tracking-wider">Email Address</span>
							<p class="text-sm font-medium">{threadDetail.thread.email}</p>
						</div>
						<div class="space-y-1">
							<span class="text-xs font-semibold text-muted-foreground uppercase tracking-wider">Category</span>
							<Badge variant="outline" class="capitalize">{threadDetail.thread.category}</Badge>
						</div>
						{#if threadDetail.thread.company_name}
							<div class="space-y-1">
								<span class="text-xs font-semibold text-muted-foreground uppercase tracking-wider">Company</span>
								<p class="text-sm font-medium">{threadDetail.thread.company_name}</p>
							</div>
						{/if}
						<div class="pt-4 border-t space-y-1">
							<span class="text-xs font-semibold text-muted-foreground uppercase tracking-wider">Thread ID</span>
							<p class="text-[10px] font-mono break-all">{threadDetail.thread.id}</p>
						</div>
					</Card.Content>
				</Card.Root>
			</div>
		</div>
	{/if}
</div>
