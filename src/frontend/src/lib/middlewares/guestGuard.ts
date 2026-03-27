import { defineMiddleware, redirect } from 'cross-router-core';

export const guestMiddleware = defineMiddleware(({ request }, next) => {
	throw redirect('/auth/login');
});
