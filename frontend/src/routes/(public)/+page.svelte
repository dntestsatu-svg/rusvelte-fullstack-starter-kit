<script lang="ts">
	import ContactForm from '$lib/components/ContactForm.svelte';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { ShoppingBag, ShieldCheck, Zap, Globe } from '@lucide/svelte';
</script>

<div class="flex flex-col min-h-screen dark bg-background text-foreground">
	<!-- Hero Section -->
	<section class="relative py-20 lg:py-32 overflow-hidden bg-background">
		<div class="container px-4 mx-auto text-center relative z-10">
			<Badge variant="outline" class="mb-6 px-4 py-1.5 text-sm font-medium border-primary/20 bg-primary/5">
				Next-Gen Merchant Platform
			</Badge>
			<h1 class="text-5xl lg:text-7xl font-extrabold tracking-tight mb-8 bg-gradient-to-b from-foreground to-foreground/70 bg-clip-text">
				Empowering Global <br /> <span class="text-primary">E-commerce Success</span>
			</h1>
			<p class="max-w-2xl mx-auto text-lg lg:text-xl text-muted-foreground mb-10 leading-relaxed">
				JustQiu provides the infrastructure you need to launch, scale, and manage your merchant empire with institutional-grade security and blazing performance.
			</p>
			<div class="flex flex-col sm:flex-row items-center justify-center gap-4">
				<Button size="lg" class="h-12 px-8 text-base shadow-lg shadow-primary/20 transition-all hover:scale-105" href="/login">
					Get Started Now
				</Button>
				<Button size="lg" variant="outline" class="h-12 px-8 text-base transition-all hover:bg-muted" href="#contact">
					Contact Sales
				</Button>
			</div>
		</div>
		
		<!-- Subtle background decoration -->
		<div class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-full h-full pointer-events-none -z-10">
			<div class="absolute top-0 right-0 w-[500px] h-[500px] bg-primary/5 rounded-full blur-3xl opacity-50 animate-pulse"></div>
			<div class="absolute bottom-0 left-0 w-[500px] h-[500px] bg-primary/10 rounded-full blur-3xl opacity-50 animate-pulse delay-1000"></div>
		</div>
	</section>

	<!-- Features/Trust Section -->
	<section class="py-20 bg-muted/30">
		<div class="container px-4 mx-auto text-center mb-16">
			<h2 class="text-3xl font-bold tracking-tight mb-4">Institutional-Grade Infrastructure</h2>
			<p class="text-muted-foreground">Trusted by merchants for its security-first architecture and modern tech stack.</p>
		</div>

		<div class="container px-4 mx-auto grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-8">
			<div class="p-8 rounded-2xl bg-card border shadow-sm hover:shadow-md transition-all">
				<div class="w-12 h-12 bg-primary/10 rounded-xl flex items-center justify-center mb-6 text-primary">
					<ShieldCheck size={28} />
				</div>
				<h3 class="text-xl font-bold mb-3">Security First</h3>
				<p class="text-muted-foreground text-sm leading-relaxed">Built with strict CSRF, Session, and Rate Limiting protections to keep your data safe.</p>
			</div>
			
			<div class="p-8 rounded-2xl bg-card border shadow-sm hover:shadow-md transition-all">
				<div class="w-12 h-12 bg-primary/10 rounded-xl flex items-center justify-center mb-6 text-primary">
					<Zap size={28} />
				</div>
				<h3 class="text-xl font-bold mb-3">Instant Scaling</h3>
				<p class="text-muted-foreground text-sm leading-relaxed">Leverage our high-performance Rust backend for sub-millisecond response times.</p>
			</div>

			<div class="p-8 rounded-2xl bg-card border shadow-sm hover:shadow-md transition-all">
				<div class="w-12 h-12 bg-primary/10 rounded-xl flex items-center justify-center mb-6 text-primary">
					<ShoppingBag size={28} />
				</div>
				<h3 class="text-xl font-bold mb-3">Merchant Focused</h3>
				<p class="text-muted-foreground text-sm leading-relaxed">Comprehensive tools for inventory, orders, and payouts management.</p>
			</div>

			<div class="p-8 rounded-2xl bg-card border shadow-sm hover:shadow-md transition-all">
				<div class="w-12 h-12 bg-primary/10 rounded-xl flex items-center justify-center mb-6 text-primary">
					<Globe size={28} />
				</div>
				<h3 class="text-xl font-bold mb-3">Global Reach</h3>
				<p class="text-muted-foreground text-sm leading-relaxed">Unified API and dashboard for managing your international operations.</p>
			</div>
		</div>
	</section>

	<!-- Contact Section -->
	<section id="contact" class="py-24 bg-background">
		<div class="container px-4 mx-auto grid grid-cols-1 lg:grid-cols-2 gap-16 items-center">
			<div class="space-y-8">
				<div class="space-y-4">
					<h2 class="text-4xl font-extrabold tracking-tight">Let's build something <br />great together</h2>
					<p class="text-muted-foreground text-lg leading-relaxed">
						Have questions about our enterprise plans or custom solutions? Our specialist support team is ready to help you optimize your merchant flow.
					</p>
				</div>
				
				<div class="space-y-6">
					<div class="flex items-center gap-4">
						<div class="w-10 h-10 bg-primary/10 rounded-lg flex items-center justify-center text-primary">
							<ShieldCheck size={20} />
						</div>
						<div>
							<h4 class="font-bold">24/7 Priority Support</h4>
							<p class="text-sm text-muted-foreground">Expert assistance whenever you need it.</p>
						</div>
					</div>
					<div class="flex items-center gap-4">
						<div class="w-10 h-10 bg-primary/10 rounded-lg flex items-center justify-center text-primary">
							<Zap size={20} />
						</div>
						<div>
							<h4 class="font-bold">Rapid Integration</h4>
							<p class="text-sm text-muted-foreground">Get up and running in minutes, not days.</p>
						</div>
					</div>
				</div>
			</div>

			<div>
				<ContactForm />
			</div>
		</div>
	</section>

	<!-- Footer -->
	<footer class="py-12 border-t bg-muted/10">
		<div class="container px-4 mx-auto flex flex-col md:flex-row justify-between items-center gap-8">
			<div class="flex items-center gap-2">
				<div class="w-8 h-8 bg-primary rounded-lg flex items-center justify-center text-primary-foreground font-bold">Q</div>
				<span class="text-xl font-bold tracking-tight">JustQiu</span>
			</div>
			<div class="text-sm text-muted-foreground">
				&copy; 2026 JustQiu. All rights reserved.
			</div>
			<div class="flex gap-6 text-sm text-muted-foreground">
				<button class="hover:text-primary transition-colors">Privacy Policy</button>
				<button class="hover:text-primary transition-colors">Terms of Service</button>
			</div>
		</div>
	</footer>
</div>
