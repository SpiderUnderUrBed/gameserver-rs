import { httpClient } from '../utils/http';

interface User {
	username: string;
}

const AUTH_STORAGE_KEY = 'auth.user';

export class AuthManager {
	public user = $state<User | null>(null);
	public readonly loggedIn = $derived(!!this.user);

	public constructor() {
		this.restoreUser();
	}

	private persistUser(): void {
		if (typeof sessionStorage === 'undefined') {
			return;
		}

		if (this.user) {
			sessionStorage.setItem(AUTH_STORAGE_KEY, JSON.stringify(this.user));
			return;
		}

		sessionStorage.removeItem(AUTH_STORAGE_KEY);
	}

	private restoreUser(): void {
		if (typeof sessionStorage === 'undefined') {
			return;
		}

		try {
			const raw = sessionStorage.getItem(AUTH_STORAGE_KEY);
			if (!raw) {
				return;
			}

			this.user = JSON.parse(raw) as User;
		} catch {
			this.user = null;
		}
	}

	public async login(username: string, password: string): Promise<void> {
		const formData = new URLSearchParams();
		formData.append('user', username);
		formData.append('password', password);

		const resp = await httpClient
			.post<{ username: string }>('/api/signin', {
				headers: {
					'Content-Type': 'application/x-www-form-urlencoded'
				},
				body: formData
			})
			.json();

		this.user = resp;
		this.persistUser();
	}

	public async fetchUser(): Promise<void> {
		const resp = await httpClient.get<{ username: string }>('/api/user/me').json();

		this.user = resp;
		this.persistUser();
	}

	public async logout(): Promise<void> {
		this.user = null;
		this.persistUser();
		await httpClient.delete<{ username: string }>('/api/signout').json();
	}
}

export const auth = new AuthManager();
