import { auth } from '../auth/auth.svelte';

export const authMiddleware = async () => {
	if (!auth.loggedIn) {
		return false;
	}
	return true;
};
