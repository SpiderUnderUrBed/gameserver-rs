import { object, unknown } from 'valibot';
import { httpClient } from '../utils/http';
import { writable } from 'svelte/store';

export type ServerStatusMode = 'node' | 'server-keyword' | 'server-process';

type ConsoleEntry = { type: 'input' | 'output'; text: string };

type NodeData = {
	nodename: string;
	ip: string;
	nodetype?: string;
	nodestatus?: { kind: string; data: unknown };
};

interface GetCurrentNodeResponse {
	nodename: string;
	ip: string;
	nodestatus: {
		kind: string;
	};
	nodetype: {
		kind: string;
	};
}

export class ServerConsoleState {
	public basePath = '';

	public consoleHistory = $state<ConsoleEntry[]>([]);
	public nodes = $state<NodeData[]>([]);
	public integrations = $state<any[]>([]);
	public selectedNode = $state<string | null>(null);
	public statusIndicator = $state<'up' | 'down' | 'unknown'>('unknown');
	public rawOutputEnabled = $state(false);
	public pendingStatus = $state<ServerStatusMode>('node');
	public finalStatus = $state<ServerStatusMode>('node');
	public isConnected = $state(false);

	// public scrollContainer: HTMLDivElement;

	private ws: WebSocket | null = null;

	constructor() {
		// nothing to do on construction, init() is explicit
	}

	public init(basePath = '') {
		this.basePath = basePath.replace(/\/$/, '');
		this.fetchNodes();
		this.fetchIntegrations();
		this.loadTopmostButtons();
		this.connectWebSocket();
		this.updateStatus('up', false);

		scrollHeight.subscribe(([height, scrollpos]) => {
			//if (height - scrollpos < 400){
				newScrollHeightEvent.set([false, height]);
			//}
		});
	}

	public addConsoleEntry(entry: ConsoleEntry) {
		this.consoleHistory = [...this.consoleHistory, entry];
		const consoleHistoryElem = document.getElementById('consoleHistory');
		if (consoleHistoryElem) {
			setTimeout(() => {
				consoleHistoryElem.scrollTop = consoleHistoryElem.scrollHeight;
			}, 1);
		}
	}

	public async fetchNodes() {
		try {
			const resp = await httpClient.get(`/api/nodes`).json<{ list?: { data: NodeData[] } }>();
			this.nodes = resp.list?.data ?? [];
		} catch (err) {
			console.error('fetchNodes error', err);
			this.nodes = [];
		}
	}

	public async fetchIntegrations() {
		try {
			const resp = await httpClient.get(`/api/intergrations`).json<{ list?: { data: any[] } }>();
			this.integrations = resp.list?.data ?? [];
		} catch (err) {
			console.error('fetchIntegrations', err);
			this.integrations = [];
		}
	}

	public connectWebSocket() {
		if (this.ws && this.ws.readyState === WebSocket.OPEN) {
			return;
		}

		try {
			this.ws = new WebSocket(`/api/ws`);
			this.ws.addEventListener('open', () => {
				this.isConnected = true;
				this.addConsoleEntry({ type: 'output', text: '[WS] connected' });
			});

			this.ws.addEventListener('message', (event) => {
				const payload = event.data;
				const out = typeof payload === 'string' ? payload : JSON.stringify(payload);
				console.log("message: " + out);
				if (this.correctMessage(out)) {
					this.addConsoleEntry({ type: 'output', text: this.cleanOutput(this.cleanJson(out)) });
				}
			});

			this.ws.addEventListener('close', () => {
				this.isConnected = false;
				this.addConsoleEntry({ type: 'output', text: '[WS] disconnected' });
				setTimeout(() => this.connectWebSocket(), 2000);
			});

			this.ws.addEventListener('error', (err) => {
				console.error('WebSocket error', err);
				this.addConsoleEntry({ type: 'output', text: '[WS] error' });
			});
		} catch (err) {
			console.error('connectWebSocket error', err);
		}
	}
	public correctMessage(input: unknown): boolean {
		let output: unknown = input;
		// if (typeof output == "string"){
		// 	return true
		// } else {
		// 	return false
		// }
		// while (
		// 	output !== null &&
		// 	(typeof output === "object" &&
		// 	"data" in output) 
		// ) {
		// }

		// if (output !== null &&
		// 	typeof output === "object" &&
		// 	"data" in output) {
		// 		return false;
		// 	} else {
		// 		return true;
		// 	}

		if (typeof input !== 'string') return true;
		
		try {
			const parsed = JSON.parse(input);
			if (parsed && typeof parsed === 'object' && 'authcode' in parsed) {
				return false;
			}
		} catch {
		}
		
		return true;

	}

	public cleanJson(input: unknown): string {
    	let output: unknown = input;

		while (
			output !== null &&
			(typeof output === "object" &&
			"data" in output) || typeof output == "string"
		) {
			let json = (output as Record<string, unknown>);
			if (typeof output == "string") {
				try {
					json = JSON.parse(output);
				} catch {
					break;
				}
			}
			let data = json.data;
			if (typeof data == "string"){
				try {
					output = JSON.parse(data);
				} catch {
					output = data;
					break;
				}
			} else {
				output = data;
			}
		}


		if (output !== null && typeof output === "object") {
			const obj = output as Record<string, unknown>;

			if (typeof obj.message === "string") {
				return obj.message;
			}

			if (typeof obj.response === "string") {
				return obj.response;
			}

			if (typeof obj.data === "string") {
				return obj.data;
			}

			return JSON.stringify(obj);
		}
		console.log("json: " + output);
		//console.log(output);
		return String(output);
	}

	public cleanOutput(str: string) {
		return str
			.replace(/\\t/g, '\t')
			.replace(/\\\\/g, '\\')
			.replace(/^\[Server\] ?/, '')
			.trim();
	}

	public async sendConsoleCommand(command: string) {
		if (!command.trim()) return;
		this.addConsoleEntry({ type: 'input', text: command });

		if (this.ws && this.ws.readyState === WebSocket.OPEN) {
			this.ws.send(command);
			return;
		}

		this.addConsoleEntry({ type: 'output', text: '[WS] not connected' });
	}

	public toggleRaw() {
		this.rawOutputEnabled = !this.rawOutputEnabled;
		this.addConsoleEntry({
			type: 'output',
			text: `Raw output ${this.rawOutputEnabled ? 'enabled' : 'disabled'}`
		});
	}

	public async updateStatus(status: 'up' | 'down' | 'none', awaitFlag: boolean) {
		if (status !== 'none') {
			this.statusIndicator = status;
		}

		try {
			const source = new EventSource(`/api/awaitserverstatus`);
			source.onmessage = (event) => {
				const data = event.data || '';
				this.addConsoleEntry({ type: 'output', text: `[STATUS] ${data}` });
			};
			source.onerror = () => {
				source.close();
			};
		} catch (err) {
			console.error('updateStatus error', err);
		}
	}

	public async startServer() {
		await this.updateStatus('up', true);
		try {
			await httpClient.post(`/api/startserver`, {
				json: {}
			});
		} catch (e) {
			console.error(e);
		}
		this.addConsoleEntry({ type: 'output', text: 'Start server called' });
	}

	public async stopServer() {
		await this.updateStatus('down', true);
		try {
			await httpClient.post(`/api/stopserver`, {
				json: {}
			});
		} catch (e) {
			console.error(e);
		}
		this.addConsoleEntry({ type: 'output', text: 'Stop server called' });
	}

	public async deleteServer(servername: string = '', authcode: string = '0') {
		console.log("deleting current server");
		try {
			await httpClient.post('/api/deleteserver', { json: { message: servername, authcode } });
			console.log("Success");
		} catch (err) {
			console.error(err);
		}
	}

	public async createDefaultServer(
		servername: string,
		provider: string,
		providertype: string,
		location: string,
		sandbox: boolean,
		authcode: string = '0'
	) {
		try {
			const node = await httpClient.get<GetCurrentNodeResponse>(`/api/getcurrentnode`, {}).json();
			await httpClient
				.post(`/api/addserver`, {
					json: {
						element: {
							kind: 'Server',
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
						require_auth: false
					}
				})
				.json();
			this.addConsoleEntry({ type: 'output', text: `Created server ${servername}` });
		} catch (err) {
			this.addConsoleEntry({ type: 'output', text: `Create server error: ${err}` });
		}
	}

	public async addNode(nodename: string, ip: string, nodetype: string) {
		try {
			const payload = {
				element: {
					kind: 'Node',
					data: {
						nodename,
						ip,
						nodetype,
						nodestatus: { kind: 'enabled', data: null },
						k8s_type: 'Unknown'
					}
				},
				jwt: '',
				require_auth: true
			};
			await httpClient.post(`/api/addnode`, { json: payload }).json();
			this.addConsoleEntry({ type: 'output', text: `Node added: ${nodename}` });
			this.fetchNodes();
		} catch (err) {
			this.addConsoleEntry({ type: 'output', text: `Add node error: ${err}` });
		}
	}

	public changeStatusType(newStatus: ServerStatusMode) {
		this.pendingStatus = newStatus;
		this.finalStatus = newStatus;
	}

	public async changeNode(server_id: string, node_id: string) {
		try {
			await httpClient.put('/api/changenode', {
				json: { server_id, node_id }
			});
		} catch (err) {
			throw new Error('Failed to change server node');
			console.error(err);
		}
	}

	public async loadTopmostButtons() {
		// In legacy app this reads from settings API; fallback defaults.
		this.addConsoleEntry({ type: 'output', text: 'Loaded topmost buttons' });
	}
}
// scrollHeight.subscribe((value) => {
// 	console.log('scrollHeight changed:', value);
// });

export const serverConsole = new ServerConsoleState();
export let scrollHeight = writable([0, 0]);
export let newScrollHeightEvent = writable<[boolean, number]>([false, 0]);