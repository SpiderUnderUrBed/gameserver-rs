import { createContext } from 'svelte';

type Theme = 'light' | 'dark' | 'system';

class ThemeState {
	private _current = $state<Theme>('system');
	private systemIsDark = $state(false);

	// Derived state: What is the actual theme being shown?
	public readonly current = $derived<Theme>(
		this._current === 'system' ? (this.systemIsDark ? 'dark' : 'light') : this._current
	);

	constructor() {
		const query = window.matchMedia('(prefers-color-scheme: dark)');
		this.systemIsDark = query.matches;

		query.addEventListener('change', (e) => {
			this.systemIsDark = e.matches;
		});

		// Load saved theme from localStorage
		const saved = localStorage.getItem('theme') as Theme;
		if (saved) this._current = saved;

		// Sync choice to localStorage and DOM
		$effect(() => {
			localStorage.setItem('theme', this._current);
			const root = document.documentElement;

			// Apply the actual theme to the HTML tag
			root.setAttribute('data-theme', this.current === 'dark' ? 'business' : 'corporate');
		});
	}

	public setTheme(newTheme: Theme) {
		this._current = newTheme;
	}

	public toggleTheme() {
		const current = this.current;
		this._current = current === 'dark' ? 'light' : 'dark';
	}
}

const [getTheme, setTheme] = createContext<ThemeState>();

export function setThemeContext() {
	return setTheme(new ThemeState());
}

export function useTheme() {
	return getTheme();
}
