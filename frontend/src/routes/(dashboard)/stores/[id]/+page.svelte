<script lang="ts">
    import { onMount } from 'svelte';
    import { page } from '$app/state';
    import { storesApi, type Store, type StoreMember } from '$lib/api/stores';
    import StoreTokensPanel from '$lib/components/stores/StoreTokensPanel.svelte';
    import { Badge } from '$lib/components/ui/badge';
    import { Button } from '$lib/components/ui/button';
    import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '$lib/components/ui/card';
    import { Input } from '$lib/components/ui/input';
    import { Label } from '$lib/components/ui/label';
    import { canAccessStoreTokens } from '$lib/stores/token-access';
    import {
        Select,
        SelectContent,
        SelectItem,
        SelectTrigger
    } from '$lib/components/ui/select';
    import {
        Table,
        TableBody,
        TableCell,
        TableHead,
        TableHeader,
        TableRow
    } from '$lib/components/ui/table';
    import {
        ChevronLeft,
        Loader2,
        Save,
        UserPlus,
        UserX
    } from '@lucide/svelte';
    import { toast } from 'svelte-sonner';

    const storeId = page.params.id ?? '';

    let store = $state<Store | null>(null);
    let members = $state<StoreMember[]>([]);
    let isLoading = $state(true);
    let isSaving = $state(false);
    let isAddingMember = $state(false);
    let memberEmail = $state('');
    let memberRole = $state<'manager' | 'staff' | 'viewer'>('staff');

    let name = $state('');
    let slug = $state('');
    let callbackUrl = $state('');
    let providerUsername = $state('');
    let status = $state<'active' | 'inactive'>('active');
    let editedRoles = $state<Record<string, StoreMember['store_role']>>({});
    let activeStoreTab = $state<'members' | 'tokens'>('members');

    const isDev = $derived(page.data.sessionUser?.role === 'dev');
    const isOwner = $derived(store?.owner_user_id === page.data.sessionUser?.id);
    const isAdmin = $derived(page.data.sessionUser?.role === 'admin');
    const canEditStore = $derived(Boolean(isDev || isOwner));
    const canManageMembers = $derived(Boolean(isDev || isOwner || isAdmin));
    const canMirrorStoreTokens = $derived(
        canAccessStoreTokens({
            storeOwnerUserId: store?.owner_user_id,
            sessionUserId: page.data.sessionUser?.id,
            sessionUserRole: page.data.sessionUser?.role
        })
    );

    $effect(() => {
        if (!canMirrorStoreTokens && activeStoreTab === 'tokens') {
            activeStoreTab = 'members';
        }
    });

    async function loadStore() {
        isLoading = true;

        try {
            const [storeResponse, membersResponse] = await Promise.all([
                storesApi.get(storeId),
                storesApi.listMembers(storeId)
            ]);

            store = storeResponse;
            members = membersResponse.members;
            name = storeResponse.name;
            slug = storeResponse.slug;
            callbackUrl = storeResponse.callback_url ?? '';
            providerUsername = storeResponse.provider_username;
            status = storeResponse.status;
            editedRoles = Object.fromEntries(
                membersResponse.members.map((member) => [member.id, member.store_role])
            );
        } catch (error) {
            console.error('Failed to load store:', error);
            toast.error(error instanceof Error ? error.message : 'Failed to load store');
        } finally {
            isLoading = false;
        }
    }

    onMount(() => {
        void loadStore();
    });

    async function handleSaveStore(event: Event) {
        event.preventDefault();
        if (!canEditStore) return;

        isSaving = true;

        try {
            const updatedStore = await storesApi.update(storeId, {
                name,
                slug,
                callback_url: callbackUrl.trim() || undefined,
                provider_username: providerUsername.trim(),
                status: isDev ? status : undefined
            });

            store = updatedStore;
            toast.success('Store updated successfully');
        } catch (error) {
            console.error('Failed to update store:', error);
            toast.error(error instanceof Error ? error.message : 'Failed to update store');
        } finally {
            isSaving = false;
        }
    }

    async function handleAddMember(event: Event) {
        event.preventDefault();
        if (!canManageMembers) return;

        isAddingMember = true;

        try {
            const member = await storesApi.addMember(storeId, {
                user_email: memberEmail.trim(),
                store_role: memberRole
            });

            members = [...members.filter((existingMember) => existingMember.id !== member.id), member];
            editedRoles = { ...editedRoles, [member.id]: member.store_role };
            memberEmail = '';
            memberRole = 'staff';
            toast.success('Member added successfully');
        } catch (error) {
            console.error('Failed to add member:', error);
            toast.error(error instanceof Error ? error.message : 'Failed to add member');
        } finally {
            isAddingMember = false;
        }
    }

    async function handleUpdateMember(member: StoreMember) {
        if (!canManageMembers) return;

        try {
            const updatedMember = await storesApi.updateMember(storeId, member.id, {
                store_role: editedRoles[member.id]
            });

            members = members.map((existingMember) =>
                existingMember.id === updatedMember.id ? updatedMember : existingMember
            );
            toast.success('Member updated successfully');
        } catch (error) {
            console.error('Failed to update member:', error);
            toast.error(error instanceof Error ? error.message : 'Failed to update member');
        }
    }

    async function handleRemoveMember(member: StoreMember) {
        if (!canManageMembers) return;
        if (!confirm(`Remove ${member.user_name} from this store?`)) return;

        try {
            await storesApi.removeMember(storeId, member.id);
            members = members.filter((existingMember) => existingMember.id !== member.id);
            toast.success('Member removed successfully');
        } catch (error) {
            console.error('Failed to remove member:', error);
            toast.error(error instanceof Error ? error.message : 'Failed to remove member');
        }
    }
</script>

<div class="space-y-6 max-w-6xl mx-auto">
    <div class="flex items-center gap-4">
        <Button variant="outline" size="icon" href="/dashboard/stores">
            <ChevronLeft class="h-4 w-4" />
        </Button>
        <div>
            <h1 class="text-3xl font-bold tracking-tight">Store Detail</h1>
            <p class="text-muted-foreground">Review tenant settings and scoped members.</p>
        </div>
    </div>

    {#if isLoading}
        <div class="flex justify-center p-12">
            <Loader2 class="h-8 w-8 animate-spin text-muted-foreground" />
        </div>
    {:else if store}
        <div class="grid gap-6 xl:grid-cols-[1.2fr_0.8fr]">
            <Card>
                <CardHeader>
                    <div class="flex items-center justify-between gap-4">
                        <div>
                            <CardTitle>{store.name}</CardTitle>
                            <CardDescription>{store.slug}</CardDescription>
                        </div>
                        <Badge variant={store.status === 'active' ? 'default' : 'secondary'} class="capitalize">
                            {store.status}
                        </Badge>
                    </div>
                </CardHeader>
                <CardContent>
                    <form class="space-y-4" onsubmit={handleSaveStore}>
                        <div class="grid gap-2">
                            <Label for="name">Store Name</Label>
                            <Input id="name" bind:value={name} disabled={!canEditStore} required />
                        </div>

                        <div class="grid gap-2">
                            <Label for="slug">Slug</Label>
                            <Input id="slug" bind:value={slug} disabled={!canEditStore} required />
                        </div>

                        <div class="grid gap-2">
                            <Label for="callback-url">Callback URL</Label>
                            <Input id="callback-url" type="url" bind:value={callbackUrl} disabled={!canEditStore} />
                        </div>

                        <div class="grid gap-2">
                            <Label for="provider-username">Provider Username</Label>
                            <Input
                                id="provider-username"
                                bind:value={providerUsername}
                                disabled={!canEditStore}
                                required
                            />
                        </div>

                        <div class="grid gap-2">
                            <Label for="status">Status</Label>
                            <Select
                                type="single"
                                value={status}
                                disabled={!isDev}
                                onValueChange={(value) => status = value as 'active' | 'inactive'}
                            >
                                <SelectTrigger id="status">
                                    <span class="capitalize">{status}</span>
                                </SelectTrigger>
                                <SelectContent>
                                    <SelectItem value="active">Active</SelectItem>
                                    <SelectItem value="inactive">Inactive</SelectItem>
                                </SelectContent>
                            </Select>
                        </div>

                        <div class="grid gap-3 rounded-lg border bg-muted/40 p-4 text-sm">
                            <div class="flex justify-between gap-4">
                                <span class="text-muted-foreground">Owner</span>
                                <div class="text-right">
                                    <div class="font-medium">{store.owner_name}</div>
                                    <div class="text-muted-foreground">{store.owner_email}</div>
                                </div>
                            </div>
                            <div class="flex justify-between gap-4">
                                <span class="text-muted-foreground">Created At</span>
                                <span>{new Date(store.created_at).toLocaleString()}</span>
                            </div>
                            <div class="flex justify-between gap-4">
                                <span class="text-muted-foreground">Updated At</span>
                                <span>{new Date(store.updated_at).toLocaleString()}</span>
                            </div>
                        </div>

                        {#if canEditStore}
                            <div class="flex justify-end pt-2">
                                <Button type="submit" disabled={isSaving}>
                                    {#if isSaving}
                                        <Loader2 class="mr-2 h-4 w-4 animate-spin" />
                                        Saving...
                                    {:else}
                                        <Save class="mr-2 h-4 w-4" />
                                        Save Store
                                    {/if}
                                </Button>
                            </div>
                        {:else}
                            <p class="rounded-md border border-dashed p-3 text-sm text-muted-foreground">
                                You have read-only access to this store.
                            </p>
                        {/if}
                    </form>
                </CardContent>
            </Card>

            <Card>
                <CardHeader>
                    <div class="flex flex-wrap items-center justify-between gap-4">
                        <div>
                            <CardTitle>{activeStoreTab === 'members' ? 'Members' : 'Tokens'}</CardTitle>
                            <CardDescription>
                                {#if activeStoreTab === 'members'}
                                    Owner is managed automatically. Additional members are scoped to this store.
                                {:else}
                                    Manage active bearer tokens for Store Client API access.
                                {/if}
                            </CardDescription>
                        </div>

                        {#if canMirrorStoreTokens}
                            <div class="inline-flex rounded-lg border bg-muted/30 p-1">
                                <button
                                    type="button"
                                    class={`rounded-md px-3 py-1.5 text-sm font-medium transition ${
                                        activeStoreTab === 'members'
                                            ? 'bg-background text-foreground shadow-sm'
                                            : 'text-muted-foreground'
                                    }`}
                                    onclick={() => activeStoreTab = 'members'}
                                >
                                    Members
                                </button>
                                <button
                                    type="button"
                                    class={`rounded-md px-3 py-1.5 text-sm font-medium transition ${
                                        activeStoreTab === 'tokens'
                                            ? 'bg-background text-foreground shadow-sm'
                                            : 'text-muted-foreground'
                                    }`}
                                    onclick={() => activeStoreTab = 'tokens'}
                                >
                                    Tokens
                                </button>
                            </div>
                        {/if}
                    </div>
                </CardHeader>
                <CardContent class="space-y-4">
                    {#if activeStoreTab === 'tokens'}
                        <StoreTokensPanel storeId={storeId} />
                    {:else}
                    {#if canManageMembers}
                        <form class="space-y-4 rounded-lg border bg-muted/40 p-4" onsubmit={handleAddMember}>
                            <div class="grid gap-2">
                                <Label for="member-email">Member Email</Label>
                                <Input
                                    id="member-email"
                                    type="email"
                                    bind:value={memberEmail}
                                    placeholder="member@example.com"
                                    required
                                />
                            </div>

                            <div class="grid gap-2">
                                <Label for="member-role">Store Role</Label>
                                <Select
                                    type="single"
                                    value={memberRole}
                                    onValueChange={(value) => memberRole = value as 'manager' | 'staff' | 'viewer'}
                                >
                                    <SelectTrigger id="member-role">
                                        <span class="capitalize">{memberRole}</span>
                                    </SelectTrigger>
                                    <SelectContent>
                                        <SelectItem value="manager">Manager</SelectItem>
                                        <SelectItem value="staff">Staff</SelectItem>
                                        <SelectItem value="viewer">Viewer</SelectItem>
                                    </SelectContent>
                                </Select>
                            </div>

                            <Button type="submit" disabled={isAddingMember} class="w-full">
                                {#if isAddingMember}
                                    <Loader2 class="mr-2 h-4 w-4 animate-spin" />
                                    Adding...
                                {:else}
                                    <UserPlus class="mr-2 h-4 w-4" />
                                    Add Member
                                {/if}
                            </Button>
                        </form>
                    {/if}

                    <div class="rounded-md border">
                        <Table>
                            <TableHeader>
                                <TableRow>
                                    <TableHead>Member</TableHead>
                                    <TableHead>Store Role</TableHead>
                                    <TableHead>Status</TableHead>
                                    <TableHead class="text-right">Actions</TableHead>
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {#if members.length === 0}
                                    <TableRow>
                                        <TableCell colspan={4} class="h-24 text-center">No members found.</TableCell>
                                    </TableRow>
                                {:else}
                                    {#each members as member}
                                        <TableRow>
                                            <TableCell>
                                                <div class="flex flex-col">
                                                    <span class="font-medium">{member.user_name}</span>
                                                    <span class="text-sm text-muted-foreground">{member.user_email}</span>
                                                </div>
                                            </TableCell>
                                            <TableCell>
                                                {#if canManageMembers && member.store_role !== 'owner'}
                                                    <Select
                                                        type="single"
                                                        value={editedRoles[member.id] ?? member.store_role}
                                                        onValueChange={(value) => {
                                                            editedRoles = {
                                                                ...editedRoles,
                                                                [member.id]: value as StoreMember['store_role']
                                                            };
                                                        }}
                                                    >
                                                        <SelectTrigger class="w-[140px]">
                                                            <span class="capitalize">{editedRoles[member.id] ?? member.store_role}</span>
                                                        </SelectTrigger>
                                                        <SelectContent>
                                                            <SelectItem value="manager">Manager</SelectItem>
                                                            <SelectItem value="staff">Staff</SelectItem>
                                                            <SelectItem value="viewer">Viewer</SelectItem>
                                                        </SelectContent>
                                                    </Select>
                                                {:else}
                                                    <Badge variant="outline" class="capitalize">{member.store_role}</Badge>
                                                {/if}
                                            </TableCell>
                                            <TableCell>
                                                <Badge variant={member.status === 'active' ? 'default' : 'secondary'} class="capitalize">
                                                    {member.status}
                                                </Badge>
                                            </TableCell>
                                            <TableCell class="text-right">
                                                {#if canManageMembers && member.store_role !== 'owner'}
                                                    <div class="flex justify-end gap-2">
                                                        <Button
                                                            variant="outline"
                                                            size="sm"
                                                            onclick={() => handleUpdateMember(member)}
                                                        >
                                                            Save
                                                        </Button>
                                                        <Button
                                                            variant="destructive"
                                                            size="sm"
                                                            onclick={() => handleRemoveMember(member)}
                                                        >
                                                            <UserX class="mr-2 h-4 w-4" />
                                                            Remove
                                                        </Button>
                                                    </div>
                                                {:else}
                                                    <span class="text-sm text-muted-foreground">Managed automatically</span>
                                                {/if}
                                            </TableCell>
                                        </TableRow>
                                    {/each}
                                {/if}
                            </TableBody>
                        </Table>
                    </div>
                    {/if}
                </CardContent>
            </Card>
        </div>
    {:else}
        <div class="rounded-lg border border-dashed p-12 text-center text-muted-foreground">
            Store not found.
        </div>
    {/if}
</div>
