<script lang="ts">
	import { page } from '$app/state';
	import DashboardPaymentDetailPage from '$lib/components/payments/DashboardPaymentDetailPage.svelte';
</script>

<DashboardPaymentDetailPage paymentId={page.params.id ?? ''} />
