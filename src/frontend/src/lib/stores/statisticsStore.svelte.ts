import { isKyError } from 'ky';
import { httpClient } from '../utils/http';
import { parseServerSentEvents } from 'parse-sse';
import * as v from 'valibot';

export type StatisticsPoint = {
	label: string;
	usedMemoryGB: number;
	totalMemoryGB: number | null;
	coreUsageHz?: number;
};

const statsSchema = v.object({
	used_memory: v.fallback(v.number(), 0),
	total_memory: v.fallback(v.number(), 0),
	core_usage: v.fallback(v.number(), 0)
});

export class StatisticsStore {
	public points = $state<StatisticsPoint[]>([]);
	public isConnected = $state(false);
	public error = $state<string | null>(null);
	public maxPoints = $state(50);

	private abortController: AbortController | null = null;

	constructor() {
		// explicit init
	}

	public init() {
		this.disconnect();
		this.points = [];
		this.error = null;
		this.connect();
	}

	public async connect() {
		if (this.abortController) {
			return;
		}

		this.abortController = new AbortController();

		try {
			const response = await httpClient('/api/statistics', {
				signal: this.abortController.signal,
				timeout: 0
			});

			if (!response.ok) {
				this.error = `statistics HTTP error ${response.status}`;
				this.isConnected = false;
				return;
			}

			this.isConnected = true;

			for await (const event of parseServerSentEvents(response)) {
				if (event.type === 'event' || event.type === 'message') {
					try {
						const payload = JSON.parse(event.data);
						this.handlePayload(payload);
					} catch (err) {
						this.error = `statistics parse error: ${err}`;
						console.error(err);
					}
				}
			}
		} catch (err) {
			if (!(err instanceof Error)) {
				this.error = `statistics connect error: ${err}`;
				return;
			}
			if (err.name === 'AbortError') {
				this.error = null;
				return;
			}

			this.error = `statistics connect error: ${err}`;
			console.error(err);
		} finally {
			this.isConnected = false;
			this.abortController = null;
		}
	}

	public disconnect() {
		this.abortController?.abort();
		this.isConnected = false;
	}

	public reset() {
		this.points = [];
		this.error = null;
		this.disconnect();
	}

	private async handlePayload(payload: unknown) {
		const data = await v.parseAsync(statsSchema, payload);
		const usedMemoryBytes = data.used_memory;
		const totalMemoryBytes = data.total_memory;
		const coreHz = data.core_usage;

		const usedMemoryGB = Number.isFinite(usedMemoryBytes) ? usedMemoryBytes / (1024 * 1024) : 0;
		const totalMemoryGB =
			totalMemoryBytes != null && Number.isFinite(Number(totalMemoryBytes))
				? Number(totalMemoryBytes) / (1024 * 1024)
				: null;

		const nextPoint: StatisticsPoint = {
			label: new Date().toLocaleTimeString(),
			usedMemoryGB,
			totalMemoryGB,
			coreUsageHz: coreHz != null ? Number(coreHz) : undefined
		};

		const copied = [...this.points, nextPoint];
		if (copied.length > this.maxPoints) {
			this.points = copied.slice(copied.length - this.maxPoints);
		} else {
			this.points = copied;
		}
	}
}
