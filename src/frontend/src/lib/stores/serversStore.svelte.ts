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

	public async addServer(
		servername: string,
		provider: string,
		providertype: string,
		location: string,
		sandbox: boolean,
		authcode: string = '0'
	) {
		this.error = null;
		try {
			let node = await httpClient.get(`/api/getcurrentnode`, {});
			await httpClient.post('/api/addserver', {
				json: {
					element: {
						kind: "Server",
						data: {
							servername,
							provider,
							providertype,
							location,
							node,
							sandbox
						}
					},
					jwt: authcode,
					require_auth: false,
				}
			});
			await this.fetchServers();
		} catch (err) {
			this.error = 'Failed to add server';
			console.error(err);
		}
	}

	public async deleteServer(servername: string = '', authcode: string = '0', delete_server_files: boolean = false) {
		console.log("deleting current server");
		try {
			await httpClient.post('/api/deleteserver', { 
				json: { 
					type: 'command', 
					message: servername, 
					authcode, 
					metadata: {
						kind: "DeleteServer",
						data: delete_server_files
					} 
				} 
			});
			console.log("Success");
		} catch (err) {
			console.error(err);
		}
	}
}

export const serversStore = new ServersStore();
