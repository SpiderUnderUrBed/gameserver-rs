import { defineMiddleware, redirect } from 'cross-router-core';
import { auth } from '../auth/auth.svelte';

export const authMiddleware = defineMiddleware(async ({}) => {
	if (!auth.loggedIn) {
		throw redirect('/auth/login');
	}
});
