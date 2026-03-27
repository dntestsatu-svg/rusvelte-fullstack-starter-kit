<script lang="ts">
	import { Menu, X, Bell, User, LayoutDashboard, Store, Users, Settings, LogOut } from "@lucide/svelte";
	import * as Sheet from "$lib/components/ui/sheet/index.js";
	import { Button } from "$lib/components/ui/button/index.js";
	import ThemeToggle from "../components/ThemeToggle.svelte";
	import NotificationBell from "$lib/components/ui/NotificationBell.svelte";

	let { children } = $props();
	let isSidebarOpen = $state(true);

	const navItems = [
		{ name: "Dashboard", href: "/dashboard", icon: LayoutDashboard },
		{ name: "Stores", href: "/dashboard/stores", icon: Store },
		{ name: "Users", href: "/dashboard/users", icon: Users },
		{ name: "Settings", href: "/dashboard/settings", icon: Settings }
	];

	function toggleSidebar() {
		isSidebarOpen = !isSidebarOpen;
	}
</script>

<div class="min-h-screen bg-background text-foreground flex overflow-hidden">
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
