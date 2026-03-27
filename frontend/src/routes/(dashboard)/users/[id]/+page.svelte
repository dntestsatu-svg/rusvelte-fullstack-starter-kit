<script lang="ts">
    import { onMount } from 'svelte';
    import { page } from '$app/state';
    import { ApiError } from '$lib/api/client';
    import { usersApi, type User } from '$lib/api/users';
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
    import { Badge } from '$lib/components/ui/badge';
    import { 
        ChevronLeft, 
        Loader2, 
        Save, 
        UserX 
    } from '@lucide/svelte';
    import { toast } from 'svelte-sonner';

    const userId = page.params.id ?? '';
    let user: User | null = $state(null);
    let isLoading = $state(true);
    let isSubmitting = $state(false);
    let isDisabling = $state(false);
    let loadError = $state('');
    let isUnauthorized = $state(false);

    // Form states
    let name = $state('');
    let email = $state('');
    let role = $state('');
    let status = $state('');

    async function fetchUser() {
        loadError = '';
        try {
            user = await usersApi.get(userId);
            if (user) {
                name = user.name;
                email = user.email;
                role = user.role;
                status = user.status;
            }
            isUnauthorized = false;
        } catch (error) {
            console.error('Failed to fetch user:', error);
            user = null;
            if (error instanceof ApiError && error.status === 403) {
                isUnauthorized = true;
            } else {
                isUnauthorized = false;
                loadError = error instanceof Error ? error.message : 'User not found';
                toast.error(loadError);
            }
        } finally {
            isLoading = false;
        }
    }

    onMount(() => {
        fetchUser();
    });

    const isDev = $derived(page.data.sessionUser?.role === 'dev');
    const isSelf = $derived(page.data.sessionUser?.id === userId);

    async function handleUpdate(e: Event) {
        e.preventDefault();
        if (!isDev) return;
        isSubmitting = true;
        try {
            await usersApi.update(userId, { name, email, role, status });
            toast.success('User updated successfully');
        } catch (error: any) {
            toast.error(error.message || 'Update failed');
        } finally {
            isSubmitting = false;
        }
    }

    async function handleDisable() {
        if (!isDev) return;
        if (!confirm('Are you sure you want to disable this user? They will be blocked from logging in.')) return;
        
        isDisabling = true;
        try {
            await usersApi.disable(userId);
            status = 'suspended';
            toast.success('User disabled successfully');
        } catch (error: any) {
            toast.error(error.message || 'Disable failed');
        } finally {
            isDisabling = false;
        }
    }
</script>

<div class="space-y-6 max-w-2xl mx-auto">
    <div class="flex items-center gap-4">
        <Button variant="outline" size="icon" href="/dashboard/users">
            <ChevronLeft class="h-4 w-4" />
        </Button>
        <h1 class="text-3xl font-bold tracking-tight">User Details</h1>
    </div>

    {#if isLoading}
        <div class="flex justify-center p-12">
            <Loader2 class="h-8 w-8 animate-spin text-muted-foreground" />
        </div>
    {:else if isUnauthorized}
        <div class="text-center p-12 bg-muted rounded-lg border border-dashed text-muted-foreground">
            You do not have permission to view this user profile.
        </div>
    {:else if loadError}
        <div class="space-y-4 text-center p-12 bg-muted rounded-lg border border-dashed">
            <p class="text-destructive">{loadError}</p>
            <Button variant="outline" onclick={() => void fetchUser()}>
                Try Again
            </Button>
        </div>
    {:else if user}
        <div class="grid gap-6">
            <Card>
                <CardHeader>
                    <div class="flex items-center justify-between">
                        <div>
                            <CardTitle>Management</CardTitle>
                            <CardDescription>Update user information or restrict access.</CardDescription>
                        </div>
                        <Badge variant={status === 'active' ? 'default' : status === 'suspended' ? 'destructive' : 'secondary'} class="capitalize">
                            {status}
                        </Badge>
                    </div>
                </CardHeader>
                <CardContent>
                    <form onsubmit={handleUpdate} class="space-y-4">
                        <div class="grid gap-2">
                            <Label for="name">Full Name</Label>
                            <Input id="name" bind:value={name} disabled={!isDev} required />
                        </div>
                        
                        <div class="grid gap-2">
                            <Label for="email">Email Address</Label>
                            <Input id="email" type="email" bind:value={email} disabled={!isDev} required />
                        </div>

                        <div class="grid gap-2">
                            <Label for="role">Platform Role</Label>
                            <Select
                                type="single"
                                value={role}
                                onValueChange={(value: string) => role = value}
                                disabled={!isDev}
                            >
                                <SelectTrigger id="role">
                                    <span class="capitalize">{role || "Select a role"}</span>
                                </SelectTrigger>
                                <SelectContent>
                                    <SelectItem value="dev">Developer</SelectItem>
                                    <SelectItem value="superadmin">Super Admin</SelectItem>
                                    <SelectItem value="admin">Admin</SelectItem>
                                    <SelectItem value="user">User</SelectItem>
                                </SelectContent>
                            </Select>
                        </div>

                        {#if isDev}
                            <div class="flex items-center justify-between pt-4 border-t gap-4">
                                <Button 
                                    variant="destructive" 
                                    type="button" 
                                    disabled={isDisabling || status === 'suspended' || isSelf}
                                    onclick={handleDisable}
                                >
                                    {#if isDisabling}
                                        <Loader2 class="mr-2 h-4 w-4 animate-spin" />
                                    {:else}
                                        <UserX class="mr-2 h-4 w-4" />
                                    {/if}
                                    Disable User
                                </Button>
                                
                                <Button type="submit" disabled={isSubmitting}>
                                    {#if isSubmitting}
                                        <Loader2 class="mr-2 h-4 w-4 animate-spin" />
                                    {:else}
                                        <Save class="mr-2 h-4 w-4" />
                                    {/if}
                                    Save Changes
                                </Button>
                            </div>
                        {:else}
                            <p class="text-sm text-yellow-600 dark:text-yellow-400 bg-yellow-50 dark:bg-yellow-900/20 p-3 rounded-md border border-yellow-200 dark:border-yellow-900/50">
                                You have read-only access to this user profile.
                            </p>
                        {/if}
                    </form>
                </CardContent>
            </Card>

            <Card>
                <CardHeader>
                    <CardTitle>System Information</CardTitle>
                </CardHeader>
                <CardContent class="grid gap-4 text-sm">
                    <div class="flex justify-between border-b pb-2">
                        <span class="text-muted-foreground">User ID</span>
                        <code class="text-xs">{user.id}</code>
                    </div>
                    <div class="flex justify-between border-b pb-2">
                        <span class="text-muted-foreground">Created At</span>
                        <span>{new Date(user.created_at).toLocaleString()}</span>
                    </div>
                    <div class="flex justify-between">
                        <span class="text-muted-foreground">Last Login</span>
                        <span>{user.last_login_at ? new Date(user.last_login_at).toLocaleString() : 'Never'}</span>
                    </div>
                </CardContent>
            </Card>
        </div>
    {:else}
        <div class="text-center p-12 bg-muted rounded-lg border border-dashed text-muted-foreground">
            User not found.
        </div>
    {/if}
</div>
