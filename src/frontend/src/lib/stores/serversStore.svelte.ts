import { httpClient } from '../utils/http';

export interface Server {
	servername: string;
}

export class ServersStore {
	public servers = $state<Server[]>([]);
	public loading = $state(false);
	public error = $state<string | null>(null);

	public async fetchServers() {
		this.loading = true;
		this.error = null;
		try {
			const response = await httpClient
				.get('/api/servers')
				.json<{ list: { kind: string; data: Server[] } }>();
			this.servers = response.list.data;
		} catch (err) {
			this.error = 'Failed to fetch servers';
			console.error(err);
		} finally {
			this.loading = false;
		}
	}

	public async addServer(servername: string, password: string, authcode: string = '0') {
		this.error = null;
		try {
			await httpClient.post('/api/createserver', {
				json: { server: servername, password, authcode }
			});
			await this.fetchServers();
		} catch (err) {
			this.error = 'Failed to add server';
			console.error(err);
		}
	}

	public async deleteServer(servername: string, authcode: string = '0') {
		this.error = null;
		try {
			await httpClient.post('/api/deleteserver', { json: { server: servername, authcode } });
			await this.fetchServers();
		} catch (err) {
			this.error = 'Failed to delete server';
			console.error(err);
		}
	}
}

export const serversStore = new ServersStore();
