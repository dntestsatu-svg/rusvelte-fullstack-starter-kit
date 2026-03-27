<script lang="ts">
	import { createMutation, createQuery, useQueryClient } from '@tanstack/svelte-query';
	import { ApiError } from '$lib/api/client';
	import { notificationsApi, type NotificationStatus } from '$lib/api/notifications';
	import { notificationQueryKeys } from '$lib/realtime/query-keys';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '$lib/components/ui/card';
	import {
		Select,
		SelectContent,
		SelectItem,
		SelectTrigger
	} from '$lib/components/ui/select';
	import { ChevronLeft, ChevronRight, MailOpen } from '@lucide/svelte';
	import { goto } from '$app/navigation';

	let currentPage = $state(1);
	let pageSize = 10;
	let statusFilter = $state<'all' | NotificationStatus>('all');
	const queryClient = useQueryClient();

	const notificationsQuery = createQuery(() => ({
		queryKey: notificationQueryKeys.list({
			page: currentPage,
			perPage: pageSize,
			status: statusFilter
		}),
		queryFn: () =>
			notificationsApi.list({
				page: currentPage,
				perPage: pageSize,
				status: statusFilter === 'all' ? undefined : statusFilter
			})
	}));

	const markReadMutation = createMutation(() => ({
		mutationFn: async (notificationId: string) => {
			await notificationsApi.markRead(notificationId);
		},
		onSuccess: async () => {
			await queryClient.invalidateQueries({ queryKey: notificationQueryKeys.all });
		}
	}));

	const notifications = $derived(notificationsQuery.data?.notifications ?? []);
	const total = $derived(notificationsQuery.data?.total ?? 0);
	const unreadCount = $derived(notificationsQuery.data?.unread_count ?? 0);
	const totalPages = $derived(Math.ceil(total / pageSize) || 1);
	const isUnauthorized = $derived(
		notificationsQuery.error instanceof ApiError && notificationsQuery.error.status === 403
	);
	const loadError = $derived(
		notificationsQuery.error instanceof ApiError && notificationsQuery.error.status !== 403
			? notificationsQuery.error.message
			: notificationsQuery.error instanceof Error
				? notificationsQuery.error.message
				: ''
	);

	function handleStatusChange(value: string) {
		statusFilter = value as 'all' | NotificationStatus;
		currentPage = 1;
	}

	function handlePageChange(nextPage: number) {
		if (nextPage < 1 || nextPage > totalPages) {
			return;
		}

		currentPage = nextPage;
	}

	async function handleMarkRead(notificationId: string, relatedType?: string | null, relatedId?: string | null) {
		await markReadMutation.mutateAsync(notificationId);

		if (relatedType === 'payment' && relatedId) {
			await goto(`/dashboard/payments/${relatedId}`);
		}
	}
</script>

<div class="space-y-6">
	<div class="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
		<div>
			<h1 class="text-3xl font-bold tracking-tight">Notifications</h1>
			<p class="text-muted-foreground">Track payment finalization updates and read them without leaving the dashboard.</p>
		</div>
		<div class="rounded-lg border bg-muted/30 px-4 py-2 text-sm text-muted-foreground">
			Unread count: <span class="font-semibold text-foreground">{unreadCount}</span>
		</div>
	</div>

	<Card>
		<CardHeader>
			<div class="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
				<div>
					<CardTitle>Notification Feed</CardTitle>
					<CardDescription>Realtime events invalidate this feed through the SSE bridge.</CardDescription>
				</div>
				<Select type="single" value={statusFilter} onValueChange={(value) => handleStatusChange(value)}>
					<SelectTrigger class="w-[180px]">
						<span class="capitalize">{statusFilter === 'all' ? 'All notifications' : statusFilter}</span>
					</SelectTrigger>
					<SelectContent>
						<SelectItem value="all">All Notifications</SelectItem>
						<SelectItem value="unread">Unread</SelectItem>
						<SelectItem value="read">Read</SelectItem>
					</SelectContent>
				</Select>
			</div>
		</CardHeader>
		<CardContent class="space-y-4">
			{#if notificationsQuery.isPending}
				<div class="rounded-lg border border-dashed p-12 text-center text-muted-foreground">
					Loading notifications...
				</div>
			{:else if isUnauthorized}
				<div class="rounded-lg border border-dashed p-12 text-center text-muted-foreground">
					You do not have permission to view notifications.
				</div>
			{:else if loadError}
				<div class="rounded-lg border border-dashed p-12 text-center text-muted-foreground">
					<div class="space-y-3">
						<p class="text-destructive">{loadError}</p>
						<Button variant="outline" size="sm" onclick={() => void notificationsQuery.refetch()}>
							Try Again
						</Button>
					</div>
				</div>
			{:else if notifications.length === 0}
				<div class="rounded-lg border border-dashed p-12 text-center text-muted-foreground">
					No notifications found for this filter.
				</div>
			{:else}
				<div class="space-y-3">
					{#each notifications as notification}
						<div class="rounded-lg border p-4">
							<div class="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
								<div class="space-y-1">
									<div class="flex items-center gap-2">
										<h2 class="font-semibold">{notification.title}</h2>
										<Badge variant={notification.status === 'unread' ? 'default' : 'outline'} class="capitalize">
											{notification.status}
										</Badge>
									</div>
									<p class="text-sm text-muted-foreground">{notification.body}</p>
									<p class="text-xs text-muted-foreground">
										{new Date(notification.created_at).toLocaleString()}
									</p>
								</div>
								<div class="flex items-center gap-2">
									{#if notification.status === 'unread'}
										<Button
											variant="outline"
											size="sm"
											disabled={markReadMutation.isPending}
											onclick={() =>
												void handleMarkRead(
													notification.id,
													notification.related_type,
													notification.related_id
												)}
										>
											<MailOpen class="mr-2 h-4 w-4" />
											Mark as read
										</Button>
									{:else if notification.related_type === 'payment' && notification.related_id}
										<Button variant="ghost" size="sm" href={`/dashboard/payments/${notification.related_id}`}>
											Open payment
										</Button>
									{/if}
								</div>
							</div>
						</div>
					{/each}
				</div>
			{/if}

			<div class="flex items-center justify-between">
				<p class="text-sm text-muted-foreground">Showing {notifications.length} of {total} notifications</p>
				<div class="flex items-center gap-2">
					<Button
						variant="outline"
						size="sm"
						disabled={currentPage === 1 || notificationsQuery.isPending}
						onclick={() => handlePageChange(currentPage - 1)}
					>
						<ChevronLeft class="h-4 w-4" />
						Previous
					</Button>
					<div class="text-sm font-medium">Page {currentPage} of {totalPages}</div>
					<Button
						variant="outline"
						size="sm"
						disabled={currentPage >= totalPages || notificationsQuery.isPending}
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
