<script lang="ts">
	import { AlertTriangle, RotateCcw } from "@lucide/svelte";
	import { Button } from "$lib/components/ui/button/index.js";
	let { 
		title = "Something went wrong", 
		message = "An unexpected error occurred while loading this data.",
		retry = null 
	} = $props();
</script>

<div class="flex flex-col items-center justify-center p-12 text-center bg-destructive/5 rounded-2xl border-2 border-destructive/20">
	<div class="h-12 w-12 rounded-full bg-destructive/10 flex items-center justify-center mb-4">
		<AlertTriangle class="h-6 w-6 text-destructive" />
	</div>
	<h3 class="text-lg font-semibold text-destructive tracking-tight">{title}</h3>
	<p class="text-sm text-muted-foreground mt-1 max-w-xs mx-auto leading-relaxed">
		{message}
	</p>
	
	{#if retry}
		<Button 
			onclick={retry}
			variant="destructive"
			class="mt-6"
		>
			<RotateCcw class="h-4 w-4 mr-2" />
			Try Again
		</Button>
	{/if}
</div>
