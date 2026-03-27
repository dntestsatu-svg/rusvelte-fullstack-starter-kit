export interface DashboardNavItem {
	id: 'dashboard' | 'stores' | 'payments' | 'users' | 'settlements' | 'inbox';
	name: string;
	href: string;
}

export function buildDashboardNav(role: string | undefined): DashboardNavItem[] {
	const items: DashboardNavItem[] = [
		{ id: 'dashboard', name: 'Dashboard', href: '/dashboard' },
		{ id: 'stores', name: 'Stores', href: '/dashboard/stores' },
		{ id: 'payments', name: 'Payments', href: '/dashboard/payments' }
	];

	if (role === 'dev' || role === 'superadmin' || role === 'admin') {
		items.splice(2, 0, { id: 'users', name: 'Users', href: '/dashboard/users' });
	}

	if (role === 'dev') {
		items.push({ id: 'settlements', name: 'Settlement Center', href: '/dashboard/settlements' });
	}

	if (role === 'dev' || role === 'superadmin') {
		items.push({ id: 'inbox', name: 'Support Inbox', href: '/dashboard/inbox' });
	}

	return items;
}
