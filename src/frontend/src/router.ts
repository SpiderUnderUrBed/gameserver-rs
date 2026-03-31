import { createBrowserRouter, type RouteDefinition } from 'cross-router-core';
import { guestMiddleware } from './lib/middlewares/guestGuard';
import Login, { action as loginAction } from './pages/Login.svelte';
import { enableDebug } from 'cross-router-svelte';
import { silentAuthMiddleware } from './lib/middlewares/silentAuthGuard';
import { authMiddleware } from './lib/middlewares/authGuard';
import Home from './pages/dashboard/Home.svelte';
import Users from './pages/dashboard/Users.svelte';
import Manual from './pages/dashboard/Manual.svelte';
import Nodes from './pages/dashboard/Nodes.svelte';
import Statistics from './pages/dashboard/home/Statistics.svelte';
import Integrations from './pages/dashboard/home/Integrations.svelte';
import DashboardRoot from './pages/dashboard/DashboardRoot.svelte';
import Workflows from './pages/dashboard/home/Workflows.svelte';
import HomeIndex from './pages/dashboard/home/HomeIndex.svelte';
import Backups from './pages/dashboard/home/Backups.svelte';
import CreateServer from './pages/dashboard/CreateServer.svelte';
import Settings from './pages/dashboard/home/Settings.svelte';

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
		component: DashboardRoot,
		children: [
			{
				id: 'home',
				path: '/',
				component: Home,
				children: [
					{
						id: 'index',
						path: '/',
						component: HomeIndex
					},
					{
						id: 'workflows',
						path: 'workflows',
						component: Workflows
					},
					{
						id: 'backups',
						path: 'backups',
						component: Backups
					},
					{
						id: 'settings',
						path: 'settings',
						component: Settings
					},
					{
						id: 'statistics',
						path: 'statistics',
						component: Statistics
					},
					{
						id: 'integrations',
						path: 'integrations',
						component: Integrations
					}
				]
			},
			{
				id: 'users',
				path: '/users',
				component: Users
			},
			{
				id: 'nodes',
				path: '/nodes',
				component: Nodes
			},
			{
				id: 'manual',
				path: '/manual',
				component: Manual
			},
			{
				id: 'create-server',
				path: '/create-server',
				component: CreateServer
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
