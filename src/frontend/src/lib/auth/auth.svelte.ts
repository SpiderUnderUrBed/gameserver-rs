import { httpClient } from '../utils/http';

interface User {}

type JwtToken = string;

export class AuthManager {
	user = $state<User | null>(null);
	jwt = $state<JwtToken | null>(null);

	public async login(username: string, password: string): Promise<void> {
		const formBody = new FormData();
		formBody.append('user', username);
		formBody.append('password', password);

		const resp = await httpClient
			.post<{ response: string }>('/api/signin', {
				headers: {
					'Content-Type': 'application/x-www-form-urlencoded'
				},
				body: formBody
			})
			.json();

		this.jwt = resp.response;
	}
}

export const auth = new AuthManager();
