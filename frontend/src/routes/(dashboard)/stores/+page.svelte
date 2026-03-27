<script lang="ts">
    import { page } from '$app/state';
    import { onMount } from 'svelte';
    import { ApiError } from '$lib/api/client';
    import { storesApi, type Store } from '$lib/api/stores';
    import { Badge } from '$lib/components/ui/badge';
    import { Button } from '$lib/components/ui/button';
    import { Input } from '$lib/components/ui/input';
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
        ChevronRight,
        Filter,
        Plus,
        Search,
        SquarePen
    } from '@lucide/svelte';
    import { toast } from 'svelte-sonner';

    let stores: Store[] = $state([]);
    let totalStores = $state(0);
    let isLoading = $state(true);
    let loadError = $state('');
    let isUnauthorized = $state(false);
    let currentPage = $state(1);
    let pageSize = 10;
    let searchQuery = $state('');
    let statusFilter = $state('all');
    let searchDebounce: ReturnType<typeof setTimeout> | null = null;

    const canCreate = $derived(page.data.sessionUser?.role === 'dev' || page.data.sessionUser?.role === 'admin');
    const totalPages = $derived(Math.ceil(totalStores / pageSize));

    async function fetchStores() {
        isLoading = true;
        loadError = '';

        try {
            const response = await storesApi.list({
                page: currentPage,
                perPage: pageSize,
                search: searchQuery.trim() || undefined,
                status: statusFilter === 'all' ? undefined : statusFilter
            });

            stores = response.stores;
            totalStores = response.total;
            isUnauthorized = false;
        } catch (error) {
            console.error('Failed to fetch stores:', error);
            if (error instanceof ApiError && error.status === 403) {
                isUnauthorized = true;
                stores = [];
                totalStores = 0;
            } else {
                isUnauthorized = false;
                loadError = error instanceof Error ? error.message : 'Failed to load stores';
                toast.error(loadError);
            }
        } finally {
            isLoading = false;
        }
    }

    onMount(() => {
        void fetchStores();
    });

    function handlePageChange(nextPage: number) {
        if (nextPage < 1 || nextPage > totalPages) {
            return;
        }

        currentPage = nextPage;
        void fetchStores();
    }

    function handleStatusChange(value: string) {
        statusFilter = value;
        currentPage = 1;
        void fetchStores();
    }

    function handleSearchInput(event: Event) {
        searchQuery = (event.currentTarget as HTMLInputElement).value;
        currentPage = 1;

        if (searchDebounce) {
            clearTimeout(searchDebounce);
        }

        searchDebounce = setTimeout(() => {
            void fetchStores();
        }, 250);
    }
</script>

<div class="space-y-6">
    <div class="flex items-center justify-between">
        <div>
            <h1 class="text-3xl font-bold tracking-tight">Stores</h1>
            <p class="text-muted-foreground">Manage tenant stores and their scoped memberships.</p>
        </div>
        {#if canCreate}
            <Button href="/dashboard/stores/new">
                <Plus class="mr-2 h-4 w-4" />
                Create Store
            </Button>
        {/if}
    </div>

    <div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div class="relative flex-1 max-w-sm">
            <Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input
                type="search"
                placeholder="Search stores, owner, slug..."
                class="pl-8"
                value={searchQuery}
                oninput={handleSearchInput}
            />
        </div>

        <div class="flex items-center gap-2">
            <Filter class="h-4 w-4 text-muted-foreground" />
            <Select
                type="single"
                value={statusFilter}
                onValueChange={(value: string) => handleStatusChange(value)}
            >
                <SelectTrigger class="w-[180px]">
                    <span class="capitalize">{statusFilter === 'all' ? 'All Statuses' : statusFilter}</span>
                </SelectTrigger>
                <SelectContent>
                    <SelectItem value="all">All Statuses</SelectItem>
                    <SelectItem value="active">Active</SelectItem>
                    <SelectItem value="inactive">Inactive</SelectItem>
                </SelectContent>
            </Select>
        </div>
    </div>

    <div class="rounded-md border bg-card">
        <Table>
            <TableHeader>
                <TableRow>
                    <TableHead>Store</TableHead>
                    <TableHead>Owner</TableHead>
                    <TableHead>Provider User</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>Updated</TableHead>
                    <TableHead class="text-right">Actions</TableHead>
                </TableRow>
            </TableHeader>
            <TableBody>
                {#if isLoading}
                    <TableRow>
                        <TableCell colspan={6} class="h-24 text-center">Loading stores...</TableCell>
                    </TableRow>
                {:else if isUnauthorized}
                    <TableRow>
                        <TableCell colspan={6} class="h-24 text-center">
                            You do not have permission to view stores.
                        </TableCell>
                    </TableRow>
                {:else if loadError}
                    <TableRow>
                        <TableCell colspan={6} class="h-24 text-center">
                            <div class="space-y-3">
                                <p class="text-destructive">{loadError}</p>
                                <Button variant="outline" size="sm" onclick={() => void fetchStores()}>
                                    Try Again
                                </Button>
                            </div>
                        </TableCell>
                    </TableRow>
                {:else if stores.length === 0}
                    <TableRow>
                        <TableCell colspan={6} class="h-24 text-center">No stores found.</TableCell>
                    </TableRow>
                {:else}
                    {#each stores as store}
                        <TableRow>
                            <TableCell>
                                <div class="flex flex-col">
                                    <span class="font-medium">{store.name}</span>
                                    <span class="text-sm text-muted-foreground">{store.slug}</span>
                                </div>
                            </TableCell>
                            <TableCell>
                                <div class="flex flex-col">
                                    <span class="font-medium">{store.owner_name}</span>
                                    <span class="text-sm text-muted-foreground">{store.owner_email}</span>
                                </div>
                            </TableCell>
                            <TableCell>{store.provider_username}</TableCell>
                            <TableCell>
                                <Badge
                                    variant={store.status === 'active' ? 'default' : 'secondary'}
                                    class="capitalize"
                                >
                                    {store.status}
                                </Badge>
                            </TableCell>
                            <TableCell>{new Date(store.updated_at).toLocaleDateString()}</TableCell>
                            <TableCell class="text-right">
                                <Button variant="ghost" size="icon" href="/dashboard/stores/{store.id}">
                                    <SquarePen class="h-4 w-4" />
                                </Button>
                            </TableCell>
                        </TableRow>
                    {/each}
                {/if}
            </TableBody>
        </Table>
    </div>

    <div class="flex items-center justify-between">
        <p class="text-sm text-muted-foreground">Showing {stores.length} of {totalStores} stores</p>
        <div class="flex items-center gap-2">
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
