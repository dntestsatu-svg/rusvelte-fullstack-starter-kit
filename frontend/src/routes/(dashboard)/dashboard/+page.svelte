<script lang="ts">
	import { Skeleton } from "$lib/components/ui/skeleton/index.js";
</script>

<div class="space-y-6">
	<div class="flex items-center justify-between">
		<div>
			<h1 class="text-3xl font-bold tracking-tight">Dashboard Overview</h1>
			<p class="text-muted-foreground mt-1">Welcome back to JustQiu Merchant Portal.</p>
		</div>
	</div>

	<div class="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
		<div class="p-6 bg-card rounded-xl border border-border/50 shadow-sm space-y-2">
			<p class="text-sm text-muted-foreground">Total Sales</p>
			<Skeleton class="h-8 w-24" />
		</div>
		<div class="p-6 bg-card rounded-xl border border-border/50 shadow-sm space-y-2">
			<p class="text-sm text-muted-foreground">Active Orders</p>
			<Skeleton class="h-8 w-16" />
		</div>
		<div class="p-6 bg-card rounded-xl border border-border/50 shadow-sm space-y-2">
			<p class="text-sm text-muted-foreground">Average Value</p>
			<Skeleton class="h-8 w-20" />
		</div>
		<div class="p-6 bg-card rounded-xl border border-border/50 shadow-sm space-y-2">
			<p class="text-sm text-muted-foreground">Growth Rate</p>
			<Skeleton class="h-8 w-12" />
		</div>
	</div>

	<div class="rounded-xl border border-border/50 bg-card overflow-hidden shadow-sm">
		<div class="p-6 border-b border-border/50">
			<h3 class="font-semibold">Recent Activity</h3>
		</div>
		<div class="p-6 space-y-4">
			<Skeleton class="h-4 w-full" />
			<Skeleton class="h-4 w-3/4" />
			<Skeleton class="h-4 w-1/2" />
		</div>
	</div>
</div>
