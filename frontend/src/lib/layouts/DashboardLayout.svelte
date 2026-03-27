<script lang="ts">
	import type { Snippet } from 'svelte';
	import {
		Menu,
		User,
		LayoutDashboard,
		Store,
		Users,
		MessageSquare,
		CreditCard,
		Landmark
	} from "@lucide/svelte";
	import * as Sheet from "$lib/components/ui/sheet/index.js";
	import { Button } from "$lib/components/ui/button/index.js";
	import ThemeToggle from "../components/ThemeToggle.svelte";
	import NotificationBell from "$lib/components/ui/NotificationBell.svelte";
	import RealtimeBridge from "$lib/realtime/RealtimeBridge.svelte";
	import type { AuthUser } from "../api/auth";
	import { buildDashboardNav, type DashboardNavItem } from '$lib/layouts/dashboard-nav';

	let { children, sessionUser } = $props<{
		children: Snippet;
		sessionUser: AuthUser;
	}>();
	let isSidebarOpen = $state(true);
	const iconMap: Record<DashboardNavItem['id'], typeof LayoutDashboard> = {
		dashboard: LayoutDashboard,
		stores: Store,
		payments: CreditCard,
		users: Users,
		settlements: Landmark,
		inbox: MessageSquare
	};

	let navItems = $derived.by(() =>
		buildDashboardNav(sessionUser?.role).map((item) => ({
			...item,
			icon: iconMap[item.id]
		}))
	);

	function toggleSidebar() {
		isSidebarOpen = !isSidebarOpen;
	}
</script>

<div class="min-h-screen bg-background text-foreground flex overflow-hidden">
	<RealtimeBridge />
	<!-- Sidebar (Desktop) -->
	<aside
		class="hidden lg:flex flex-col bg-muted/30 border-r transition-all duration-300 ease-in-out"
		style:width={isSidebarOpen ? "260px" : "80px"}
	>
		<div class="h-16 flex items-center px-6 border-b">
			<span class="font-bold text-xl text-primary overflow-hidden whitespace-nowrap">
				{isSidebarOpen ? "JustQiu" : "JQ"}
			</span>
		</div>

		<nav class="flex-1 py-6 space-y-2 px-3">
			{#each navItems as item}
				<Button
					variant="ghost"
					href={item.href}
					class="w-full justify-start gap-4 px-4 py-3"
				>
					<item.icon class="w-5 h-5 text-muted-foreground" />
					{#if isSidebarOpen}
						<span class="font-medium text-foreground">{item.name}</span>
					{/if}
				</Button>
			{/each}
		</nav>

		<div class="p-4 border-t">
			<Button
				variant="ghost"
				class="w-full justify-start gap-4"
				onclick={toggleSidebar}
			>
				<Menu class="w-5 h-5" />
				{#if isSidebarOpen}
					<span>Collapse</span>
				{/if}
			</Button>
		</div>
	</aside>

	<!-- Main Content Area -->
	<main class="flex-1 flex flex-col relative overflow-hidden">
		<!-- Topbar -->
		<header class="h-16 border-b flex items-center justify-between px-6 bg-background/80 backdrop-blur-md z-10">
			<div class="flex items-center">
				<Sheet.Root>
					<Sheet.Trigger>
						{#snippet child({ props })}
							<Button variant="ghost" size="icon-sm" class="lg:hidden" {...props}>
								<Menu />
							</Button>
						{/snippet}
					</Sheet.Trigger>
					<Sheet.Content side="left" class="w-64 p-0">
						<div class="h-16 flex items-center px-6 border-b">
							<span class="font-bold text-xl text-primary">JustQiu</span>
						</div>
						<nav class="py-6 space-y-2 px-3">
							{#each navItems as item}
								<Button
									variant="ghost"
									href={item.href}
									class="w-full justify-start gap-4"
								>
									<item.icon class="w-5 h-5 text-muted-foreground" />
									<span class="font-medium">{item.name}</span>
								</Button>
							{/each}
						</nav>
					</Sheet.Content>
				</Sheet.Root>
			</div>

			<div class="flex items-center space-x-4">
				<NotificationBell />
				<ThemeToggle />
				<Button variant="ghost" size="icon-sm" class="rounded-none bg-primary/10 border border-primary/20">
					<User class="text-primary" />
				</Button>
			</div>
		</header>

		<!-- Page Content -->
		<div class="flex-1 overflow-y-auto p-8">
			<div class="max-w-7xl mx-auto">
				{@render children()}
			</div>
		</div>
	</main>
</div>
