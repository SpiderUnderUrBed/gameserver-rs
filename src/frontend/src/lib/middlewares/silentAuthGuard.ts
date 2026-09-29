import { auth } from '../auth/auth.svelte';

export const silentAuthMiddleware = async () => {
	try {
		await auth.fetchUser();
	} catch {}
	return true;
};
