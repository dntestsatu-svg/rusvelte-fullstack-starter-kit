<script lang="ts">
	import { page } from "$app/state";
	import { protectRoute } from "$lib/auth/guard.js";
	import DashboardLayout from "$lib/layouts/DashboardLayout.svelte";

	let { children } = $props();

	// Protect all routes in this group
	$effect(() => {
		protectRoute(page.url);
	});
</script>

<DashboardLayout>
	{@render children()}
</DashboardLayout>
