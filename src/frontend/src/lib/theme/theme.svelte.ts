import { createContext } from 'svelte';

type Theme = 'light' | 'dark' | 'system';

class ThemeState {
	public current = $state<Theme>('system');

	private systemIsDark = $state(false);

	constructor() {
		const query = window.matchMedia('(prefers-color-scheme: dark)');
		this.systemIsDark = query.matches;

		query.addEventListener('change', (e) => {
			this.systemIsDark = e.matches;
		});

		// Load saved theme from localStorage
		const saved = localStorage.getItem('theme') as Theme;
		if (saved) this.current = saved;

		// Sync choice to localStorage and DOM
		$effect(() => {
			localStorage.setItem('theme', this.current);
			const root = document.documentElement;

			// Apply the actual theme to the HTML tag
			root.setAttribute('data-theme', this.resolved === 'dark' ? 'business' : 'corporate');
		});
	}

	// Derived state: What is the actual theme being shown?
	private get resolved(): Theme {
		if (this.current === 'system') {
			return this.systemIsDark ? 'dark' : 'light';
		}
		return this.current;
	}

	public setTheme(newTheme: Theme) {
		this.current = newTheme;
	}

	public toggleTheme() {
		const current = this.resolved;
		this.current = current === 'dark' ? 'light' : 'dark';
	}
}

const [getTheme, setTheme] = createContext<ThemeState>();

export function setThemeContext() {
	return setTheme(new ThemeState());
}

export function useTheme() {
	return getTheme();
}
