<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import * as Select from '$lib/components/ui/select';
	import { toast } from 'svelte-sonner';
	import { apiFetch } from '$lib/api/client';

	let name = $state('');
	let email = $state('');
	let subject = $state('');
	let category = $state('');
	let message = $state('');
	let isSubmitting = $state(false);

	const categories = [
		{ value: 'sales', label: 'Sales Inquiry' },
		{ value: 'support', label: 'Technical Support' },
		{ value: 'billing', label: 'Billing' },
		{ value: 'other', label: 'Other' }
	];

	async function handleSubmit(e: SubmitEvent) {
		e.preventDefault();
		if (isSubmitting) return;

		if (!name || !email || !subject || !category || !message) {
			toast.error('Please fill in all fields');
			return;
		}

		isSubmitting = true;
		try {
			await apiFetch('/api/v1/public/contact', {
				method: 'POST',
				body: JSON.stringify({
					name,
					email,
					subject,
					category,
					message,
					captcha_token: 'dev-pass' // Local/Dev verification token
				})
			});

			toast.success('Message sent successfully!');
			name = '';
			email = '';
			subject = '';
			category = '';
			message = '';
		} catch (err) {
			toast.error('An error occurred. Please try again later.');
		} finally {
			isSubmitting = false;
		}
	}
</script>

<form onsubmit={handleSubmit} class="space-y-6 max-w-lg mx-auto p-6 border rounded-xl bg-card shadow-sm dark">
	<div class="space-y-2">
		<h2 class="text-2xl font-bold tracking-tight">Contact Us</h2>
		<p class="text-sm text-muted-foreground">We'll get back to you as soon as possible.</p>
	</div>

	<div class="grid grid-cols-1 md:grid-cols-2 gap-4">
		<div class="space-y-2">
			<Label for="name">Name</Label>
			<Input id="name" bind:value={name} placeholder="John Doe" required />
		</div>
		<div class="space-y-2">
			<Label for="email">Email</Label>
			<Input id="email" type="email" bind:value={email} placeholder="john@example.com" required />
		</div>
	</div>

	<div class="space-y-2">
		<Label for="category">Category</Label>
		<Select.Root
			type="single"
			bind:value={category}
		>
			<Select.Trigger class="w-full">
				<span>{categories.find((item) => item.value === category)?.label ?? 'Select a category'}</span>
			</Select.Trigger>
			<Select.Content>
				{#each categories as cat}
					<Select.Item value={cat.value}>{cat.label}</Select.Item>
				{/each}
			</Select.Content>
		</Select.Root>
	</div>

	<div class="space-y-2">
		<Label for="subject">Subject</Label>
		<Input id="subject" bind:value={subject} placeholder="How can we help?" required />
	</div>

	<div class="space-y-2">
		<Label for="message">Message</Label>
		<Textarea
			id="message"
			bind:value={message}
			placeholder="Tell us more about your inquiry..."
			class="min-h-[120px]"
			required
		/>
	</div>

	<Button type="submit" class="w-full" disabled={isSubmitting}>
		{isSubmitting ? 'Sending...' : 'Send Message'}
	</Button>
</form>
