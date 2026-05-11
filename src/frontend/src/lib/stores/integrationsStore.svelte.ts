import type { EnumTypeSimple } from '../utils/common';
import { httpClient } from '../utils/http';

export interface Integration {
	type: Record<string, any>;
	status: string;
	settings: Record<string, EnumTypeSimple>;
}

export class IntegrationsStore {
	public integrations = $state<Integration[]>([]);
	public loading = $state(false);
	public error = $state<string | null>(null);

	public async fetchIntegrations() {
		this.loading = true;
		this.error = null;
		try {
			const response = await httpClient
				.get('/api/intergrations')
				.json<{ list: { kind: string; data: Integration[] } }>();
			this.integrations = response.list.data;
		} catch (err) {
			this.error = 'Failed to fetch integrations';
			console.error(err);
		} finally {
			this.loading = false;
		}
	}

	public async createIntegration(type: EnumTypeSimple, status: string, settings: Record<string, any> = {}) {
		this.error = null;
		try {
			const payload = {
				element: {
					kind: 'Intergration',
					data: { status, type, settings }
				},
				jwt: '',
				require_auth: false
			};
			console.log(payload);
			await httpClient.post('/api/createintergrations', { json: payload });
			await this.fetchIntegrations();
		} catch (err) {
			this.error = 'Failed to create integration';
			console.error(err);
		}
	}

	public async modifyIntegration(type: EnumTypeSimple, status: string, settings: Record<string, any> = {}) {
		this.error = null;
		try {
			const payload = {
				element: {
					kind: 'Intergration',
					data: { status, type, settings }
				},
				jwt: '',
				require_auth: false
			};
			console.log(payload);
			await httpClient.post('/api/modifyintergrations', { json: payload });
			await this.fetchIntegrations();
		} catch (err) {
			this.error = 'Failed to modify integration';
			console.error(err);
		}
	}

	public async deleteIntegration(name: string) {
		this.error = null;
		try {
			const payload = {
				element: {
					kind: 'Intergration',
					data: { status: "disabled", type: { kind: name }, settings: {} }
				},
				jwt: '',
				require_auth: false
			};
			await httpClient.post('/api/deleteintergrations', { json: payload });
			await this.fetchIntegrations();
		} catch (err) {
			this.error = 'Failed to delete integration';
			console.error(err);
		}
	}
}

export const integrationsStore = new IntegrationsStore();
