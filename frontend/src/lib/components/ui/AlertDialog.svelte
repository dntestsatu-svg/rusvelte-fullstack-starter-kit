<script lang="ts">
	import * as AlertDialog from "$lib/components/ui/alert-dialog/index.js";
	let { 
		open = $bindable(false), 
		title = "System Alert", 
		message = "An important event has occurred."
	} = $props();
</script>

<AlertDialog.Root bind:open>
	<AlertDialog.Content>
		<AlertDialog.Header>
			<AlertDialog.Title>{title}</AlertDialog.Title>
			<AlertDialog.Description>
				{message}
			</AlertDialog.Description>
		</AlertDialog.Header>
		<AlertDialog.Footer>
			<AlertDialog.Action onclick={() => open = false}>Acknowledge</AlertDialog.Action>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>
