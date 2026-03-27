<script lang="ts">
	import "../app.css";
	import { QueryClient, QueryClientProvider } from "@tanstack/svelte-query";
	import { onNavigate } from "$app/navigation";
	import { ModeWatcher } from "mode-watcher";
	import { Toaster } from "$lib/components/ui/sonner/index.js";
	import nprogress from "nprogress";
	import "nprogress/nprogress.css";
	import TopLoadingBar from "$lib/components/TopLoadingBar.svelte";

	let { children } = $props();

	const queryClient = new QueryClient({
		defaultOptions: {
			queries: {
				staleTime: 1000 * 60 * 5, // 5 minutes
				refetchOnWindowFocus: false,
			},
		},
	});

	// Configure NProgress
	nprogress.configure({ showSpinner: false });

	onNavigate((navigation) => {
		if (navigation.type !== "link") return;
		nprogress.start();

		navigation.complete.finally(() => {
			nprogress.done();
		});
	});
</script>

<ModeWatcher />
<Toaster />
<TopLoadingBar />

<QueryClientProvider client={queryClient}>
	{@render children()}
</QueryClientProvider>
