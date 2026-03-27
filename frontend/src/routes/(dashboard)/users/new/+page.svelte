<script lang="ts">
    import { page } from '$app/state';
    import { usersApi } from '$lib/api/users';
    import { Button } from '$lib/components/ui/button';
    import { Input } from '$lib/components/ui/input';
    import { Label } from '$lib/components/ui/label';
    import { 
        Select, 
        SelectContent, 
        SelectItem, 
        SelectTrigger
    } from '$lib/components/ui/select';
    import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '$lib/components/ui/card';
    import { 
        ChevronLeft, 
        Loader2, 
        Save 
    } from '@lucide/svelte';
    import { goto } from '$app/navigation';
    import { toast } from 'svelte-sonner';

    let name = $state('');
    let email = $state('');
    let password = $state('');
    let role = $state('user');
    let isSubmitting = $state(false);
    const canCreate = $derived(page.data.sessionUser?.role === 'dev');

    async function handleSubmit(e: Event) {
        e.preventDefault();
        if (!canCreate) return;
        isSubmitting = true;
        
        try {
            await usersApi.create({ name, email, password, role });
            toast.success('User created successfully');
            await goto('/dashboard/users');
        } catch (error: any) {
            toast.error(error.message || 'Failed to create user');
        } finally {
            isSubmitting = false;
        }
    }
</script>

<div class="space-y-6 max-w-2xl mx-auto">
    <div class="flex items-center gap-4">
        <Button variant="outline" size="icon" href="/dashboard/users">
            <ChevronLeft class="h-4 w-4" />
        </Button>
        <h1 class="text-3xl font-bold tracking-tight">Add User</h1>
    </div>

    {#if !canCreate}
        <Card>
            <CardHeader>
                <CardTitle>Access Denied</CardTitle>
                <CardDescription>Only dev users can create platform users in v1.</CardDescription>
            </CardHeader>
            <CardContent>
                <p class="text-sm text-muted-foreground">
                    You can return to the users list or ask a dev user to perform this action.
                </p>
            </CardContent>
        </Card>
    {:else}
    <Card>
        <CardHeader>
            <CardTitle>User Details</CardTitle>
            <CardDescription>Enter the information for the new platform user.</CardDescription>
        </CardHeader>
        <CardContent>
            <form onsubmit={handleSubmit} class="space-y-4">
                <div class="grid gap-2">
                    <Label for="name">Full Name</Label>
                    <Input id="name" placeholder="John Doe" bind:value={name} required />
                </div>
                
                <div class="grid gap-2">
                    <Label for="email">Email Address</Label>
                    <Input id="email" type="email" placeholder="john@example.com" bind:value={email} required />
                </div>

                <div class="grid gap-2">
                    <Label for="password">Password</Label>
                    <Input id="password" type="password" placeholder="••••••••" bind:value={password} required />
                </div>

                <div class="grid gap-2">
                    <Label for="role">Platform Role</Label>
                    <Select type="single" value={role} onValueChange={(value: string) => role = value}>
                        <SelectTrigger id="role">
                            <span class="capitalize">{role || "Select a role"}</span>
                        </SelectTrigger>
                        <SelectContent>
                            <SelectItem value="superadmin">Super Admin</SelectItem>
                            <SelectItem value="admin">Admin</SelectItem>
                            <SelectItem value="user">User</SelectItem>
                        </SelectContent>
                    </Select>
                    <p class="text-xs text-muted-foreground mt-1">
                        Roles define the capabilities and scoping of the user.
                    </p>
                </div>

                <div class="flex justify-end gap-3 pt-4">
                    <Button variant="outline" href="/dashboard/users" disabled={isSubmitting}>
                        Cancel
                    </Button>
                    <Button type="submit" disabled={isSubmitting}>
                        {#if isSubmitting}
                            <Loader2 class="mr-2 h-4 w-4 animate-spin" />
                            Creating...
                        {:else}
                            <Save class="mr-2 h-4 w-4" />
                            Create User
                        {/if}
                    </Button>
                </div>
            </form>
        </CardContent>
    </Card>
    {/if}
</div>
