<script lang="ts">
    import { page } from '$app/state';
    import { goto } from '$app/navigation';
    import { storesApi } from '$lib/api/stores';
    import { Button } from '$lib/components/ui/button';
    import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '$lib/components/ui/card';
    import { Input } from '$lib/components/ui/input';
    import { Label } from '$lib/components/ui/label';
    import { ChevronLeft, Loader2, Save } from '@lucide/svelte';
    import { toast } from 'svelte-sonner';

    let ownerEmail = $state('');
    let name = $state('');
    let slug = $state('');
    let callbackUrl = $state('');
    let providerUsername = $state('');
    let isSubmitting = $state(false);
    let slugTouched = $state(false);
    const canCreate = $derived(page.data.sessionUser?.role === 'dev' || page.data.sessionUser?.role === 'admin');

    function generateSlug(value: string) {
        return value
            .trim()
            .toLowerCase()
            .replace(/[^a-z0-9]+/g, '-')
            .replace(/^-+|-+$/g, '');
    }

    function handleNameInput(event: Event) {
        name = (event.currentTarget as HTMLInputElement).value;
        if (!slugTouched) {
            slug = generateSlug(name);
        }
    }

    function handleSlugInput(event: Event) {
        slugTouched = true;
        slug = generateSlug((event.currentTarget as HTMLInputElement).value);
    }

    async function handleSubmit(event: Event) {
        event.preventDefault();
        if (!canCreate) return;
        isSubmitting = true;

        try {
            const store = await storesApi.create({
                owner_email: ownerEmail.trim(),
                name,
                slug,
                callback_url: callbackUrl.trim() || undefined,
                provider_username: providerUsername.trim()
            });

            toast.success('Store created successfully');
            await goto(`/dashboard/stores/${store.id}`);
        } catch (error) {
            console.error('Failed to create store:', error);
            toast.error(error instanceof Error ? error.message : 'Failed to create store');
        } finally {
            isSubmitting = false;
        }
    }
</script>

<div class="space-y-6 max-w-3xl mx-auto">
    <div class="flex items-center gap-4">
        <Button variant="outline" size="icon" href="/dashboard/stores">
            <ChevronLeft class="h-4 w-4" />
        </Button>
        <div>
            <h1 class="text-3xl font-bold tracking-tight">Create Store</h1>
            <p class="text-muted-foreground">Create a tenant store and assign its primary owner.</p>
        </div>
    </div>

    {#if !canCreate}
    <Card>
        <CardHeader>
            <CardTitle>Access Denied</CardTitle>
            <CardDescription>Only dev and admin users can create stores in v1.</CardDescription>
        </CardHeader>
        <CardContent>
            <p class="text-sm text-muted-foreground">
                You can still browse stores you are allowed to access from the stores list.
            </p>
        </CardContent>
    </Card>
    {:else}
    <Card>
        <CardHeader>
            <CardTitle>Store Details</CardTitle>
            <CardDescription>The owner will automatically receive the <code>owner</code> store role.</CardDescription>
        </CardHeader>
        <CardContent>
            <form class="space-y-4" onsubmit={handleSubmit}>
                <div class="grid gap-2">
                    <Label for="owner-email">Owner Email</Label>
                    <Input
                        id="owner-email"
                        type="email"
                        placeholder="owner@example.com"
                        bind:value={ownerEmail}
                        required
                    />
                </div>

                <div class="grid gap-2">
                    <Label for="name">Store Name</Label>
                    <Input
                        id="name"
                        placeholder="Example Store"
                        value={name}
                        oninput={handleNameInput}
                        required
                    />
                </div>

                <div class="grid gap-2">
                    <Label for="slug">Store Slug</Label>
                    <Input
                        id="slug"
                        placeholder="example-store"
                        value={slug}
                        oninput={handleSlugInput}
                        required
                    />
                </div>

                <div class="grid gap-2">
                    <Label for="callback-url">Callback URL</Label>
                    <Input
                        id="callback-url"
                        type="url"
                        placeholder="https://merchant.example.com/callback"
                        bind:value={callbackUrl}
                    />
                </div>

                <div class="grid gap-2">
                    <Label for="provider-username">Provider Username</Label>
                    <Input
                        id="provider-username"
                        placeholder="provider-store-user"
                        bind:value={providerUsername}
                        required
                    />
                </div>

                <div class="flex justify-end gap-3 pt-4">
                    <Button variant="outline" href="/dashboard/stores" disabled={isSubmitting}>
                        Cancel
                    </Button>
                    <Button type="submit" disabled={isSubmitting}>
                        {#if isSubmitting}
                            <Loader2 class="mr-2 h-4 w-4 animate-spin" />
                            Creating...
                        {:else}
                            <Save class="mr-2 h-4 w-4" />
                            Create Store
                        {/if}
                    </Button>
                </div>
            </form>
        </CardContent>
    </Card>
    {/if}
</div>
