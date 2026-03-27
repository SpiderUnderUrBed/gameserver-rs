import { createBrowserRouter, type RouteDefinition } from 'cross-router-core';
import { guestMiddleware } from './lib/middlewares/guestGuard';
import Login, { action as loginAction } from './pages/Login.svelte';

const routes: RouteDefinition[] = [
	{
		id: 'auth',
		path: 'auth',
		children: [
			{
				id: 'login',
				path: '/login',
				component: Login,
				action: loginAction
			}
		]
	},
	{
		id: 'root',
		path: '/',
		middleware: [guestMiddleware],
		children: [
			{
				id: 'home',
				path: '/'
			}
		]
	}
];

export const router = createBrowserRouter({}, routes);
