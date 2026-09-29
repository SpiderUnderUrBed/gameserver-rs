import { auth } from '../auth/auth.svelte';

export const guestMiddleware = async () => {
	if (auth.loggedIn) {
		return false;
	}
	return true;
};
