import { httpClient } from '../utils/http';

export interface Node {
	nodename: string;
}

export class NodesStore {
	public nodes = $state<Node[]>([]);
	public loading = $state(false);
	public error = $state<string | null>(null);

	public async fetchNodes() {
		this.loading = true;
		this.error = null;
		try {
			const response = await httpClient
				.get('/api/nodes')
				.json<{ list: { kind: string; data: Node[] } }>();
			this.nodes = response.list.data;
		} catch (err) {
			this.error = 'Failed to fetch nodes';
			console.error(err);
		} finally {
			this.loading = false;
		}
	}
	
	public async addNode(nodename: string, password: string, authcode: string = '0') {
		this.error = null;
		try {
			await httpClient.post('/api/addnode', { json: { node: nodename, password, authcode } });
			await this.fetchNodes();
		} catch (err) {
			this.error = 'Failed to add node';
			console.error(err);
		}
	}

	public async deleteNode(nodename: string, authcode: string = '0') {
		this.error = null;
		try {
			await httpClient.post('/api/deletenode', { json: { node: nodename, authcode } });
			await this.fetchNodes();
		} catch (err) {
			this.error = 'Failed to delete node';
			console.error(err);
		}
	}
}

export const nodesStore = new NodesStore();
