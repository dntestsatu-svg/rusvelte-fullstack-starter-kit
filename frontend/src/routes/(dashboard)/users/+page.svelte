<script lang="ts">
    import { page } from '$app/state';
    import { onMount } from 'svelte';
    import { ApiError } from '$lib/api/client';
    import { usersApi, type User } from '$lib/api/users';
    import { Button } from '$lib/components/ui/button';
    import { Input } from '$lib/components/ui/input';
    import { 
        Table, 
        TableBody, 
        TableCell, 
        TableHead, 
        TableHeader, 
        TableRow 
    } from '$lib/components/ui/table';
    import { 
        Select, 
        SelectContent, 
        SelectItem, 
        SelectTrigger
    } from '$lib/components/ui/select';
    import { Badge } from '$lib/components/ui/badge';
    import { 
        Users, 
        Search, 
        UserPlus, 
        Filter, 
        ChevronLeft, 
        ChevronRight,
        UserCog
    } from '@lucide/svelte';
    import { toast } from 'svelte-sonner';

    let users: User[] = $state([]);
    let totalUsers = $state(0);
    let isLoading = $state(true);
    let loadError = $state('');
    let isUnauthorized = $state(false);
    let currentPage = $state(1);
    let pageSize = 10;
    let roleFilter = $state('all');
    let searchQuery = $state('');
    let searchDebounce: ReturnType<typeof setTimeout> | null = null;

    async function fetchUsers() {
        isLoading = true;
        loadError = '';
        try {
            const res = await usersApi.list({ 
                page: currentPage,
                perPage: pageSize,
                role: roleFilter === 'all' ? undefined : roleFilter,
                search: searchQuery.trim() || undefined
            });
            users = res.users;
            totalUsers = res.total;
            isUnauthorized = false;
        } catch (error) {
            console.error('Failed to fetch users:', error);
            if (error instanceof ApiError && error.status === 403) {
                isUnauthorized = true;
                users = [];
                totalUsers = 0;
            } else {
                isUnauthorized = false;
                loadError = error instanceof Error ? error.message : 'Failed to load users';
                toast.error(loadError);
            }
        } finally {
            isLoading = false;
        }
    }

    onMount(() => {
        fetchUsers();
    });

    const canCreate = $derived(page.data.sessionUser?.role === 'dev');
    const totalPages = $derived(Math.ceil(totalUsers / pageSize));

    function handlePageChange(newPage: number) {
        if (newPage >= 1 && newPage <= totalPages) {
            currentPage = newPage;
            fetchUsers();
        }
    }

    function handleFilterChange(val: string) {
        roleFilter = val;
        currentPage = 1;
        fetchUsers();
    }

    function handleSearchInput(event: Event) {
        searchQuery = (event.currentTarget as HTMLInputElement).value;
        currentPage = 1;

        if (searchDebounce) {
            clearTimeout(searchDebounce);
        }

        searchDebounce = setTimeout(() => {
            void fetchUsers();
        }, 250);
    }
</script>

<div class="space-y-6">
    <div class="flex items-center justify-between">
        <div>
            <h1 class="text-3xl font-bold tracking-tight">Users</h1>
            <p class="text-muted-foreground">Manage platform users and their roles.</p>
        </div>
        {#if canCreate}
            <Button href="/dashboard/users/new">
                <UserPlus class="mr-2 h-4 w-4" />
                Add User
            </Button>
        {/if}
    </div>

    <div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div class="relative flex-1 max-w-sm">
            <Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input
                type="search"
                placeholder="Search users..."
                class="pl-8"
                value={searchQuery}
                oninput={handleSearchInput}
            />
        </div>
        
        <div class="flex items-center gap-2">
            <Filter class="h-4 w-4 text-muted-foreground" />
            <Select
                type="single"
                onValueChange={(value: string) => handleFilterChange(value)}
                value={roleFilter}
            >
                <SelectTrigger class="w-[180px]">
                    <span class="capitalize">{roleFilter === 'all' ? 'All Roles' : roleFilter}</span>
                </SelectTrigger>
                <SelectContent>
                    <SelectItem value="all">All Roles</SelectItem>
                    <SelectItem value="dev">Developer</SelectItem>
                    <SelectItem value="superadmin">Super Admin</SelectItem>
                    <SelectItem value="admin">Admin</SelectItem>
                    <SelectItem value="user">User</SelectItem>
                </SelectContent>
            </Select>
        </div>
    </div>

    <div class="rounded-md border bg-card">
        <Table>
            <TableHeader>
                <TableRow>
                    <TableHead>User</TableHead>
                    <TableHead>Role</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>Created</TableHead>
                    <TableHead class="text-right">Actions</TableHead>
                </TableRow>
            </TableHeader>
            <TableBody>
                {#if isLoading}
                    <TableRow>
                        <TableCell colspan={5} class="h-24 text-center">Loading users...</TableCell>
                    </TableRow>
                {:else if isUnauthorized}
                    <TableRow>
                        <TableCell colspan={5} class="h-24 text-center">
                            You do not have permission to view platform users.
                        </TableCell>
                    </TableRow>
                {:else if loadError}
                    <TableRow>
                        <TableCell colspan={5} class="h-24 text-center">
                            <div class="space-y-3">
                                <p class="text-destructive">{loadError}</p>
                                <Button variant="outline" size="sm" onclick={() => void fetchUsers()}>
                                    Try Again
                                </Button>
                            </div>
                        </TableCell>
                    </TableRow>
                {:else if users.length === 0}
                    <TableRow>
                        <TableCell colspan={5} class="h-24 text-center">No users found.</TableCell>
                    </TableRow>
                {:else}
                    {#each users as u}
                        <TableRow>
                            <TableCell>
                                <div class="flex flex-col">
                                    <span class="font-medium">{u.name}</span>
                                    <span class="text-sm text-muted-foreground">{u.email}</span>
                                </div>
                            </TableCell>
                            <TableCell>
                                <Badge variant="outline" class="capitalize">{u.role}</Badge>
                            </TableCell>
                            <TableCell>
                                <Badge 
                                    variant={u.status === 'active' ? 'default' : u.status === 'suspended' ? 'destructive' : 'secondary'}
                                    class="capitalize"
                                >
                                    {u.status}
                                </Badge>
                            </TableCell>
                            <TableCell>{new Date(u.created_at).toLocaleDateString()}</TableCell>
                            <TableCell class="text-right">
                                <Button variant="ghost" size="icon" href="/dashboard/users/{u.id}">
                                    <UserCog class="h-4 w-4" />
                                </Button>
                            </TableCell>
                        </TableRow>
                    {/each}
                {/if}
            </TableBody>
        </Table>
    </div>

    <div class="flex items-center justify-between">
        <p class="text-sm text-muted-foreground">
            Showing {users.length} of {totalUsers} users
        </p>
        <div class="flex items-center space-x-2">
            <Button
                variant="outline"
                size="sm"
                onclick={() => handlePageChange(currentPage - 1)}
                disabled={currentPage === 1 || isLoading}
            >
                <ChevronLeft class="h-4 w-4" />
                Previous
            </Button>
            <div class="text-sm font-medium">Page {currentPage} of {totalPages || 1}</div>
            <Button
                variant="outline"
                size="sm"
                onclick={() => handlePageChange(currentPage + 1)}
                disabled={currentPage === totalPages || isLoading || totalPages === 0}
            >
                Next
                <ChevronRight class="h-4 w-4" />
            </Button>
        </div>
    </div>
</div>
