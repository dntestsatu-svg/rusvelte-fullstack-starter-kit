import { describe, expect, it } from 'vitest';

import { buildDashboardNav } from './dashboard-nav';

describe('buildDashboardNav', () => {
	it('shows settlement center only for dev', () => {
		expect(buildDashboardNav('dev').map((item) => item.id)).toContain('settlements');
		expect(buildDashboardNav('superadmin').map((item) => item.id)).not.toContain('settlements');
		expect(buildDashboardNav('admin').map((item) => item.id)).not.toContain('settlements');
		expect(buildDashboardNav('user').map((item) => item.id)).not.toContain('settlements');
	});

	it('keeps inbox for dev and superadmin only', () => {
		expect(buildDashboardNav('dev').map((item) => item.id)).toContain('inbox');
		expect(buildDashboardNav('superadmin').map((item) => item.id)).toContain('inbox');
		expect(buildDashboardNav('admin').map((item) => item.id)).not.toContain('inbox');
	});
});
