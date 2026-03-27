import { redirect } from '@sveltejs/kit';
import type { LayoutServerLoad } from './$types';
import { buildLoginRedirectPath, fetchSessionUser, SESSION_DEPENDENCY } from '$lib/auth/session';
import { buildBackendUrl } from '$lib/server/backend';

export const load: LayoutServerLoad = async ({ depends, request, url }) => {
	depends(SESSION_DEPENDENCY);

	const sessionUser = await fetchSessionUser({
		fetch: globalThis.fetch,
		request: buildBackendUrl('/api/v1/auth/me'),
		cookieHeader: request.headers.get('cookie')
	});

	if (!sessionUser) {
		throw redirect(303, buildLoginRedirectPath(url));
	}

	return {
		sessionUser
	};
};
