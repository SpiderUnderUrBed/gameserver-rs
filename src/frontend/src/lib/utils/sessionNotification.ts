function getSessionStore(): Storage | undefined {
	if (typeof globalThis !== 'undefined' && 'sessionStorage' in globalThis && globalThis.sessionStorage) {
		return globalThis.sessionStorage;
	}

	const store = new Map<string, string>();
	const fallback: Storage = {
		length: 0,
		clear: () => {
			store.clear();
		},
		getItem: (name: string) => store.get(name) ?? null,
		key: (index: number) => Array.from(store.keys())[index] ?? null,
		removeItem: (name: string) => {
			store.delete(name);
		},
		setItem: (name: string, value: string) => {
			store.set(name, String(value));
		}
	};

	Object.defineProperty(globalThis, 'sessionStorage', {
		value: fallback,
		configurable: true,
		writable: true
	});

	return fallback;
}

export function shouldNotifyOnce(key: string): boolean {
	const storage = getSessionStore();
	if (!storage) {
		return true;
	}

	const raw = storage.getItem('session-notifications');
	const seen = new Set<string>(raw ? JSON.parse(raw) as string[] : []);

	if (seen.has(key)) {
		return false;
	}

	seen.add(key);
	storage.setItem('session-notifications', JSON.stringify([...seen]));
	return true;
}
