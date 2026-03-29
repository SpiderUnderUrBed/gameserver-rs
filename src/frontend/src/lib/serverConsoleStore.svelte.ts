import { httpClient } from './utils/http';

export type ServerStatusMode = 'node' | 'server-keyword' | 'server-process';

type ConsoleEntry = { type: 'input' | 'output'; text: string };

type NodeData = {
	nodename: string;
	ip: string;
	nodetype?: string;
	nodestatus?: { kind: string; data: unknown };
};

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
	public nodePanelVisible = $state(false);
	public toggablePagesVisible = $state(false);
	public managementButtonsVisible = $state(false);
	public isConnected = $state(false);

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
			const resp = await httpClient
				.get(`${this.basePath}/api/nodes`)
				.json<{ list?: { data: NodeData[] } }>();
			this.nodes = resp.list?.data ?? [];
		} catch (err) {
			console.error('fetchNodes error', err);
			this.nodes = [];
		}
	}

	public async fetchIntegrations() {
		try {
			const resp = await httpClient
				.get(`${this.basePath}/api/intergrations`)
				.json<{ list?: { data: any[] } }>();
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
			this.ws = new WebSocket(`${this.basePath}/api/ws`);
			this.ws.addEventListener('open', () => {
				this.isConnected = true;
				this.addConsoleEntry({ type: 'output', text: '[WS] connected' });
			});

			this.ws.addEventListener('message', (event) => {
				const payload = event.data;
				const out = typeof payload === 'string' ? payload : JSON.stringify(payload);
				this.addConsoleEntry({ type: 'output', text: this.cleanOutput(out) });
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
			const source = new EventSource(`${this.basePath}/api/awaitserverstatus`);
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
		this.addConsoleEntry({ type: 'output', text: 'Start server called' });
	}

	public async stopServer() {
		await this.updateStatus('down', true);
		this.addConsoleEntry({ type: 'output', text: 'Stop server called' });
	}

	public async createDefaultServer(
		servername: string,
		provider: string,
		location: string,
		sandbox: boolean
	) {
		try {
			const payload = { servername, provider, location, sandbox };
			await httpClient.post(`${this.basePath}/api/createserver`, { json: payload }).json();
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
			await httpClient.post(`${this.basePath}/api/nodes`, { json: payload }).json();
			this.addConsoleEntry({ type: 'output', text: `Node added: ${nodename}` });
			this.fetchNodes();
		} catch (err) {
			this.addConsoleEntry({ type: 'output', text: `Add node error: ${err}` });
		}
	}

	public toggleNodes() {
		this.nodePanelVisible = !this.nodePanelVisible;
	}

	public toggleDeveloperOptions() {
		this.toggablePagesVisible = !this.toggablePagesVisible;
	}

	public toggleManagementButtons() {
		this.managementButtonsVisible = !this.managementButtonsVisible;
	}

	public changeStatusType(newStatus: ServerStatusMode) {
		this.pendingStatus = newStatus;
		this.finalStatus = newStatus;
	}

	public async loadTopmostButtons() {
		// In legacy app this reads from settings API; fallback defaults.
		this.addConsoleEntry({ type: 'output', text: 'Loaded topmost buttons' });
	}
}

export const serverConsole = new ServerConsoleState();
