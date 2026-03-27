<script lang="ts">
	import { createQuery } from '@tanstack/svelte-query';
	import { Bell } from "@lucide/svelte";
	import { notificationsApi } from '$lib/api/notifications';
	import { notificationQueryKeys } from '$lib/realtime/query-keys';
	import { Button } from "$lib/components/ui/button/index.js";

	const unreadQuery = createQuery(() => ({
		queryKey: notificationQueryKeys.bell(),
		queryFn: () => notificationsApi.list({ page: 1, perPage: 5 }),
		staleTime: 30_000
	}));

	const unreadCount = $derived(unreadQuery.data?.unread_count ?? 0);
</script>

<div class="relative">
	<Button variant="ghost" size="icon" class="relative" href="/dashboard/notifications">
		<Bell class="h-5 w-5 text-muted-foreground" />
		{#if unreadCount > 0}
			<span class="absolute top-1 right-1 flex h-4 w-4 items-center justify-center rounded-full bg-primary text-[10px] font-bold text-primary-foreground">
				{unreadCount > 9 ? "9+" : unreadCount}
			</span>
		{/if}
	</Button>
</div>
