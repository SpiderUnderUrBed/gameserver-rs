import { httpClient } from '../utils/http';

export interface User {
	username: string;
	user_perms: string[];
}

export class UserStore {
	public users = $state<User[]>([]);
	public loading = $state(false);
	public error = $state<string | null>(null);

	public async fetchUsers() {
		this.loading = true;
		this.error = null;
		try {
			const response = await httpClient
				.get('/api/users')
				.json<{ list: { kind: string; data: User[] } }>();
			this.users = response.list.data;
		} catch (err) {
			this.error = 'Failed to fetch users';
			console.error(err);
		} finally {
			this.loading = false;
		}
	}

	public async createUser(username: string, password: string, user_perms: string[]) {
		this.error = null;
		try {
			const payload = {
				element: {
					kind: 'User',
					data: { password, user: username, user_perms }
				},
				jwt: '',
				require_auth: true
			};
			await httpClient.post('/api/createuser', { json: payload });
			await this.fetchUsers();
		} catch (err) {
			this.error = 'Failed to create user';
			console.error(err);
		}
	}

	public async editUser(username: string, password?: string, user_perms?: string[]) {
		this.error = null;
		try {
			const payload = {
				element: {
					kind: 'User',
					data: { password: password || '', user: username, user_perms: user_perms || [] }
				},
				jwt: '',
				require_auth: true
			};
			await httpClient.post('/api/edituser', { json: payload });
			await this.fetchUsers();
		} catch (err) {
			this.error = 'Failed to edit user';
			console.error(err);
		}
	}

	public async deleteUser(username: string) {
		this.error = null;
		try {
			const payload = {
				element: {
					kind: 'User',
					data: { password: '', user: username, user_perms: [] }
				},
				jwt: '',
				require_auth: true
			};
			await httpClient.post('/api/deleteuser', { json: payload });
			await this.fetchUsers();
		} catch (err) {
			this.error = 'Failed to delete user';
			console.error(err);
		}
	}

	public async getUser(username: string): Promise<User | null> {
		try {
			const response = await httpClient
				.post('/api/getuser', { json: { user: username } })
				.json<{ user_perms: string[] }>();
			return { username, user_perms: response.user_perms };
		} catch (err) {
			console.error(err);
			return null;
		}
	}
}

export const userStore = new UserStore();
