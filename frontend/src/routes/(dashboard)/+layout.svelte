<script lang="ts">
	import { browser } from "$app/environment";
	import { authStore } from "$lib/auth/store";
	import DashboardLayout from "$lib/layouts/DashboardLayout.svelte";

let { children, data } = $props();

	$effect(() => {
		if (!browser) {
			return;
		}

		authStore.setAuth(data.sessionUser);
	});
</script>

<DashboardLayout sessionUser={data.sessionUser}>
	{@render children()}
</DashboardLayout>
