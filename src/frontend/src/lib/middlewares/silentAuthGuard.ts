import { defineMiddleware } from 'cross-router-core';
import { auth } from '../auth/auth.svelte';

export const silentAuthMiddleware = defineMiddleware(async ({}) => {
	try {
		await auth.fetchUser();
	} catch {}
});
