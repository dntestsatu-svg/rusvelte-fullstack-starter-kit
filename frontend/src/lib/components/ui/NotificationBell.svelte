<script lang="ts">
	import { Bell } from "@lucide/svelte";
	import { Button } from "$lib/components/ui/button/index.js";
	let { unreadCount = 0 } = $props();
</script>

<div class="relative">
	<Button variant="ghost" size="icon" class="relative">
		<Bell class="h-5 w-5 text-muted-foreground" />
		{#if unreadCount > 0}
			<span class="absolute top-1 right-1 flex h-4 w-4 items-center justify-center rounded-full bg-primary text-[10px] font-bold text-primary-foreground">
				{unreadCount > 9 ? "9+" : unreadCount}
			</span>
		{/if}
	</Button>
</div>
