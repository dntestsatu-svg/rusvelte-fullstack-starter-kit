<script lang="ts">
	import { browser } from '$app/environment';
	import { onMount } from 'svelte';
	import { useQueryClient } from '@tanstack/svelte-query';

	import { invalidateRealtimeQueries, reconnectDelay, type RealtimeEventName } from './events';
	import { startRealtimeBridge } from './bridge';

	const queryClient = useQueryClient();

	function parseEventPayload(event: MessageEvent<string>): unknown {
		try {
			return JSON.parse(event.data);
		} catch {
			return event.data;
		}
	}

	onMount(() => {
		if (!browser) {
			return;
		}

		return startRealtimeBridge({
			createSource: () => new EventSource('/api/v1/realtime/stream', { withCredentials: true }),
			onEvent: async (type: RealtimeEventName, event: MessageEvent<string>) => {
				await invalidateRealtimeQueries(queryClient, {
					type,
					data: parseEventPayload(event)
				});
			},
			reconnectDelay
		});
	});
</script>
