<script lang="ts">
	import { toast } from "svelte-sonner";
	import ConfirmDialog from "$lib/components/ui/ConfirmDialog.svelte";
	import AlertDialog from "$lib/components/ui/AlertDialog.svelte";
	import { Skeleton } from "$lib/components/ui/skeleton/index.js";
	import { Button } from "$lib/components/ui/button/index.js";

	let confirmOpen = $state(false);
	let alertOpen = $state(false);

	function showSuccessToast() {
		toast.success("Shadcn-Sonner is working!", {
			description: "This is a programmatic toast trigger."
		});
	}

	function showErrorToast() {
		toast.error("Operation failed", {
			description: "Example error toast."
		});
	}
</script>

<div class="space-y-12">
	<section>
		<h2 class="text-3xl font-bold tracking-tight mb-6">shadcn-svelte + Tailwind v4 Proof</h2>
		<div class="flex flex-wrap gap-4">
			<Button onclick={() => confirmOpen = true}>Test Confirm Dialog</Button>
			<Button variant="secondary" onclick={() => alertOpen = true}>Test Alert Dialog</Button>
			<Button variant="outline" onclick={showSuccessToast}>Test Success Toast</Button>
			<Button variant="destructive" onclick={showErrorToast}>Test Error Toast</Button>
		</div>
	</section>

	<section class="space-y-4">
		<h3 class="text-xl font-semibold">Skeleton Primitives</h3>
		<div class="flex items-center space-x-4">
			<Skeleton class="h-12 w-12 rounded-full" />
			<div class="space-y-2">
				<Skeleton class="h-4 w-[250px]" />
				<Skeleton class="h-4 w-[200px]" />
			</div>
		</div>
	</section>

	<section class="space-y-4">
		<h3 class="text-xl font-semibold">Button Variants</h3>
		<div class="flex flex-wrap gap-2">
			<Button>Default</Button>
			<Button variant="secondary">Secondary</Button>
			<Button variant="destructive">Destructive</Button>
			<Button variant="outline">Outline</Button>
			<Button variant="ghost">Ghost</Button>
		</div>
	</section>
</div>

<ConfirmDialog 
	bind:open={confirmOpen} 
	title="Execute Action?"
	description="Are you sure you want to test the shadcn-based ConfirmDialog?"
	onConfirm={() => toast.success("Action confirmed")}
/>

<AlertDialog 
	bind:open={alertOpen} 
	title="System Alert"
	message="This is a system alert wrapper around shadcn AlertDialog."
/>
