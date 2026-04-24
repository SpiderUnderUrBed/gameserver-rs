import { httpClient } from '../utils/http';

interface User {
	username: string;
}

export class AuthManager {
	public user = $state<User | null>(null);
	public readonly loggedIn = $derived(!!this.user);

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
	}

	public async fetchUser(): Promise<void> {
		const resp = await httpClient.get<{ username: string }>('/api/user/me').json();

		this.user = resp;
	}

	public async logout(): Promise<void> {
		this.user = null;
		await httpClient.delete<{ username: string }>('/api/signout').json();
	}
}

export const auth = new AuthManager();
