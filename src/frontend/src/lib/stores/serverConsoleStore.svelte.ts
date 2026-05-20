import { metadata, object, unknown } from 'valibot';
import { httpClient } from '../utils/http';
import { get, writable } from 'svelte/store';
import type { Filters } from './settingsStore.svelte';
import type { Filter } from '@lucide/svelte';

//export const statusStore = writable<('manual', 'server'), ('up' | 'down' | 'unknown')>(('manual'), ('unknown'));
export const statusStore = writable<'up' | 'down' | 'unknown'>('unknown');
export const statusType = writable<'node-status' | 'server-keyword' | 'server-process' | 'manual-click'>('manual-click');

export type ServerStatusMode = 'node' | 'server-keyword' | 'server-process';

type ConsoleEntry = { type: 'input' | 'output'; text: string; count: number };

type NodeData = {
	nodename: string;
	ip: string;
	nodetype?: string;
	nodestatus?: { kind: string; data: unknown };
};
type ServerData = {
	servername: string
	provider: string,
	providertype: string,
	location: string,
	node: NodeData,
	sandbox: boolean
}

export interface GetCurrentNodeResponse {
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

	public currentWsEntry = writable('');
	public consoleHistory = $state<ConsoleEntry[]>([]);
	public nodes = $state<NodeData[]>([]);
	public servers = $state<ServerData[]>([]);
	public integrations = $state<any[]>([]);
	public selectedNode = $state<string | null>(null);
	public selectedServer = $state<string | null>(null);
	public filters = $state<Filters[] | undefined>();
	//public statusIndicator = $state<'up' | 'down' | 'unknown'>('unknown');
	public rawOutputEnabled = $state(false);
	public pendingStatus = $state<ServerStatusMode>('node');
	public finalStatus = $state<ServerStatusMode>('node');
	public isConnected = $state(false);
	public duplicateAmount = writable(0);
	private oldMessage = $state('');
	private pendingEntries: ConsoleEntry[] = [];
	private flushTimer: ReturnType<typeof setTimeout> | null = null;


	// public scrollContainer: HTMLDivElement;

	private ws: WebSocket | null = null;

	constructor() {
		// nothing to do on construction, init() is explicit
	}

	public init(basePath = '') {
		this.basePath = basePath.replace(/\/$/, '');
		this.fetchNodes();
		this.fetchServers();
		this.fetchCurrentServer();
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
	public async fetchServers(): Promise<ServerData[] | undefined> {
		try {
			const response = await httpClient
				.get('/api/servers')
				.json<{ list?: { data: ServerData[] } }>();

			this.servers = response.list?.data ?? [];
			return this.servers;
		} catch (e) {
			console.error(e);
		}
	}
	public async fetchCurrentServer(){
		try {
			let response = await httpClient.post("/api/getserver", { json: { element: "" } }).json<ServerData>();
			console.log(response);
			this.selectedServer = response.servername;
		} catch (e) {
			console.error(e);
		}
	}

	public addConsoleEntry(entry: ConsoleEntry) {
	    this.pendingEntries.push(entry);
		if (this.flushTimer) clearTimeout(this.flushTimer);
		this.flushTimer = setTimeout(() => {
			this.consoleHistory = [...this.consoleHistory, ...this.pendingEntries];
			this.pendingEntries = [];
			this.flushTimer = null;
		}, 16);
	}

	public async fetchNodes() : Promise<NodeData[]> {
		try {
			const resp = await httpClient.get(`/api/nodes`).json<{ list?: { data: NodeData[] } }>();
			this.nodes = resp.list?.data ?? [];
			return this.nodes;
		} catch (err) {
			console.error('fetchNodes error', err);
			this.nodes = [];
			return this.nodes;
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
				this.addConsoleEntry({
					type: 'output', text: '[WS] connected',
					count: 0
				});
			});

			this.ws.addEventListener('message', (event) => {
				const payload = event.data;
				const out = typeof payload === 'string' ? payload : JSON.stringify(payload);
				console.log("message: " + out);
				this.currentWsEntry.set(out);
				if (this.correctMessage(out)) {
					const filtered = this.filterMessage(this.filters, this.cleanOutput(this.cleanJson(out)));
					if (filtered !== "") {
						this.addConsoleEntry({ type: 'output', text: filtered, count: 0 });
					}
				}
			});

			this.ws.addEventListener('close', () => {
				this.isConnected = false;
				this.addConsoleEntry({
					type: 'output', text: '[WS] disconnected',
					count: 0
				});
				setTimeout(() => this.connectWebSocket(), 2000);
			});

			this.ws.addEventListener('error', (err) => {
				console.error('WebSocket error', err);
				this.addConsoleEntry({
					type: 'output', text: '[WS] error',
					count: 0
				});
			});
		} catch (err) {
			console.error('connectWebSocket error', err);
		}
	}
	public filterMessage(filters: Filters[] | undefined, message: string): string {
		if (filters) {
			let final_message = message;
			for (const filter of filters) {
				if (filter.kind == "terminal") {
					final_message = final_message.replace(/(\x1b|\u009b)?\[[0-9;?]*[a-zA-Z]/g, '');
				}
				if (filter.kind == "duplicates") {
					if (this.oldMessage === final_message) {
						const lastPending = this.pendingEntries.at(-1);
						if (lastPending) {
							lastPending.count++;
						} else {
							const lastHistory = this.consoleHistory.at(-1);
							if (lastHistory) {
								lastHistory.count++;
								this.consoleHistory = [...this.consoleHistory];
							}
						}
						this.oldMessage = final_message;
						return "";
					}
				}
			}
			this.oldMessage = final_message;
			return final_message;
		} else {
			return message;
		}
	}
	public correctMessage(input: unknown): boolean {
		let output: unknown = input;

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
		this.addConsoleEntry({
			type: 'input', text: command,
			count: 0
		});

		if (this.ws && this.ws.readyState === WebSocket.OPEN) {
			this.ws.send(command);
			return;
		}

		this.addConsoleEntry({
			type: 'output', text: '[WS] not connected',
			count: 0
		});
	}

	public toggleRaw() {
		this.rawOutputEnabled = !this.rawOutputEnabled;
		this.addConsoleEntry({
			type: 'output',
			text: `Raw output ${this.rawOutputEnabled ? 'enabled' : 'disabled'}`,
			count: 0
		});
	}

	public async updateStatus(status: 'up' | 'down' | 'unknown', awaitFlag: boolean) {
		if (get(statusType) == "manual-click") {
			console.log(get(statusType));
			statusStore.set(status);
		}

		try {
			const source = new EventSource(`/api/awaitserverstatus`);
			source.onmessage = (event) => {
				const data = event.data || 'unknown';
				if (get(statusType) != "manual-click") {
					statusStore.set(data);
				}
				//this.addConsoleEntry({ type: 'output', text: `[STATUS] ${data}` });
			};
			source.onerror = () => {
				source.close();
			};
		} catch (err) {
			console.error('updateStatus error', err);
		}
	}
	public async setServer(servername: string){
		try {
			let response = await httpClient.post(`/api/setserver`, {
				json: {
					element: {
						kind: "String",
						data: servername
					},
					jwt: "",
					require_auth: true

				}
			});
			if (response.ok) {
				this.selectedServer = servername;
			}
		} catch (e) {
			console.error(e);
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
		this.addConsoleEntry({
			type: 'output', text: 'Start server called',
			count: 0
		});
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
		this.addConsoleEntry({
			type: 'output', text: 'Stop server called',
			count: 0
		});
	}

	public async deleteServer(servername: string = '', authcode: string = '0', delete_server_files: boolean = false) {
		console.log("deleting current server");
		try {
			await httpClient.post('/api/deleteserver', { 
				json: { 
					type: 'command', 
					message: '', 
					authcode, 
					metadata: {
						kind: "DeleteServer",
						data: {
							delete_server_name: servername, 
        					delete_server_files
						}
					} 
				} 
			});
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
			this.addConsoleEntry({
				type: 'output', text: `Created server ${servername}`,
				count: 0
			});
		} catch (err) {
			this.addConsoleEntry({
				type: 'output', text: `Create server error: ${err}`,
				count: 0
			});
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
			this.addConsoleEntry({
				type: 'output', text: `Node added: ${nodename}`,
				count: 0
			});
			this.fetchNodes();
		} catch (err) {
			this.addConsoleEntry({
				type: 'output', text: `Add node error: ${err}`,
				count: 0
			});
		}
	}
	public async deleteNode(nodename: string, ip: string, nodetype: string) {
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
			await httpClient.post(`/api/deletenode`, { json: payload }).json();
			this.addConsoleEntry({
				type: 'output', text: `Node deleted: ${nodename}`,
				count: 0
			});
			this.fetchNodes();
		} catch (err) {
			console.log(err);
			this.addConsoleEntry({
				type: 'output', text: `Add node error: ${err}`,
				count: 0
			});
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
		this.addConsoleEntry({
			type: 'output', text: 'Loaded topmost buttons',
			count: 0
		});
	}
}
// scrollHeight.subscribe((value) => {
// 	console.log('scrollHeight changed:', value);
// });

export const serverConsole = new ServerConsoleState();
export let scrollHeight = writable([0, 0]);
export let newScrollHeightEvent = writable<[boolean, number]>([false, 0]);