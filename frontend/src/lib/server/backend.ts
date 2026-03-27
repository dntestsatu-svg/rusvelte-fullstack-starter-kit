import { env } from '$env/dynamic/private';

function trimTrailingSlash(value: string): string {
	return value.endsWith('/') ? value.slice(0, -1) : value;
}

export function getBackendOrigin(): string {
	const explicitOrigin =
		env.BACKEND_INTERNAL_ORIGIN ?? env.INTERNAL_API_ORIGIN ?? env.BACKEND_ORIGIN;

	if (explicitOrigin) {
		return trimTrailingSlash(explicitOrigin);
	}

	return 'http://127.0.0.1:8080';
}

export function buildBackendUrl(pathname: string): string {
	return new URL(pathname, `${getBackendOrigin()}/`).toString();
}
