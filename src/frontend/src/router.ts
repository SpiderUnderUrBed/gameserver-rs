import { createBrowserRouter, type RouteDefinition } from 'cross-router-core';
import { guestMiddleware } from './lib/middlewares/guestGuard';
import Login, { action as loginAction } from './pages/Login.svelte';
import { enableDebug } from 'cross-router-svelte';
import { silentAuthMiddleware } from './lib/middlewares/silentAuthGuard';
import { authMiddleware } from './lib/middlewares/authGuard';

enableDebug();

const routes: RouteDefinition[] = [
	{
		id: 'auth',
		path: 'auth',
		middleware: [guestMiddleware],
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
		middleware: [authMiddleware],
		children: [
			{
				id: 'home',
				path: '/'
			}
		]
	}
];

export const router = createBrowserRouter(
	{
		middleware: [silentAuthMiddleware]
	},
	routes
);
