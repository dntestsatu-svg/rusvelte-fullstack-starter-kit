<script lang="ts">
	import * as AlertDialog from "$lib/components/ui/alert-dialog/index.js";
	let { 
		open = $bindable(false), 
		title = "Are you sure?", 
		description = "This action cannot be undone.",
		onConfirm = () => {},
		onCancel = () => {}
	}: {
		open?: boolean;
		title?: string;
		description?: string;
		onConfirm?: () => void;
		onCancel?: () => void;
	} = $props();
</script>

<AlertDialog.Root bind:open>
	<AlertDialog.Content>
		<AlertDialog.Header>
			<AlertDialog.Title>{title}</AlertDialog.Title>
			<AlertDialog.Description>
				{description}
			</AlertDialog.Description>
		</AlertDialog.Header>
		<AlertDialog.Footer>
			<AlertDialog.Cancel onclick={() => onCancel()}>Cancel</AlertDialog.Cancel>
			<AlertDialog.Action onclick={() => onConfirm()}>Confirm</AlertDialog.Action>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>
