<script lang="ts">
    import { page } from '$app/state';
    import { authApi } from '$lib/api/auth';
    import { resolvePostLoginPath } from '$lib/auth/session';
    import { authStore } from '$lib/auth/store';
    import { Button } from '$lib/components/ui/button';
    import { Input } from '$lib/components/ui/input';
    import { Label } from '$lib/components/ui/label';
    import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from '$lib/components/ui/card';
    import { Loader2, LogIn } from '@lucide/svelte';
    import { toast } from 'svelte-sonner';

    let email = $state('');
    let password = $state('');
    let isSubmitting = $state(false);

    async function handleLogin(e: Event) {
        e.preventDefault();
        isSubmitting = true;
        
        try {
            const response = await authApi.login({ email, password, captcha_token: 'dev-pass' });
            authStore.setAuth(response.user);
            toast.success('Welcome back!');
            window.location.assign(resolvePostLoginPath(page.url));
        } catch (error: any) {
            toast.error(error.message || 'Login failed. Please check your credentials.');
        } finally {
            isSubmitting = false;
        }
    }
</script>

<div class="min-h-screen flex items-center justify-center bg-muted/40 p-4">
    <Card class="w-full max-w-md">
        <CardHeader class="space-y-1">
            <CardTitle class="text-2xl font-bold">Login</CardTitle>
            <CardDescription>
                Enter your email and password to access your account
            </CardDescription>
        </CardHeader>
        <CardContent>
            <form onsubmit={handleLogin} class="space-y-4">
                <div class="grid gap-2">
                    <Label for="email">Email</Label>
                    <Input id="email" type="email" placeholder="m@example.com" bind:value={email} required />
                </div>
                <div class="grid gap-2">
                    <div class="flex items-center justify-between">
                        <Label for="password">Password</Label>
                    </div>
                    <Input id="password" type="password" bind:value={password} required />
                </div>
                <Button type="submit" class="w-full" disabled={isSubmitting}>
                    {#if isSubmitting}
                        <Loader2 class="mr-2 h-4 w-4 animate-spin" />
                        Logging in...
                    {:else}
                        <LogIn class="mr-2 h-4 w-4" />
                        Login
                    {/if}
                </Button>
            </form>
        </CardContent>
        <CardFooter>
            <p class="text-xs text-center w-full text-muted-foreground">
                JustQiu Admin Platform &bull; v0.1.0
            </p>
        </CardFooter>
    </Card>
</div>
