<script lang="ts">
	import { Inbox } from "@lucide/svelte";
	let { title = "No data found", message = "Try adjusting your filters or search terms." } = $props();
</script>

<div class="flex flex-col items-center justify-center p-12 text-center bg-muted/20 rounded-2xl border-2 border-dashed border-muted">
	<div class="h-12 w-12 rounded-full bg-muted flex items-center justify-center mb-4">
		<Inbox class="h-6 w-6 text-muted-foreground" />
	</div>
	<h3 class="text-lg font-semibold tracking-tight">{title}</h3>
	<p class="text-sm text-muted-foreground mt-1 max-w-xs mx-auto leading-relaxed">
		{message}
	</p>
</div>
