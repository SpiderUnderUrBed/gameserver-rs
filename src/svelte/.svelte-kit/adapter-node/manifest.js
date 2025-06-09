export const manifest = (() => {
function __memo(fn) {
	let value;
	return () => value ??= (value = fn());
}

return {
	appDir: "_app",
	appPath: "_app",
	assets: new Set(["favicon.png","index.html","main.css","main.html","main.js","users.html.old","users.js.old"]),
	mimeTypes: {".png":"image/png",".html":"text/html",".css":"text/css",".js":"text/javascript"},
	_: {
		client: {start:"_app/immutable/entry/start.742ezgxF.js",app:"_app/immutable/entry/app.CYT4Is_S.js",imports:["_app/immutable/entry/start.742ezgxF.js","_app/immutable/chunks/Dn3LHCXO.js","_app/immutable/chunks/DvKonxjB.js","_app/immutable/chunks/CfQV25Rc.js","_app/immutable/entry/app.CYT4Is_S.js","_app/immutable/chunks/DvKonxjB.js","_app/immutable/chunks/lCEkFFCo.js","_app/immutable/chunks/Df-Lx6fs.js","_app/immutable/chunks/CfQV25Rc.js"],stylesheets:[],fonts:[],uses_env_dynamic_public:false},
		nodes: [
			__memo(() => import('./nodes/0.js')),
			__memo(() => import('./nodes/1.js')),
			__memo(() => import('./nodes/2.js'))
		],
		routes: [
			{
				id: "/",
				pattern: /^\/$/,
				params: [],
				page: { layouts: [0,], errors: [1,], leaf: 2 },
				endpoint: null
			}
		],
		prerendered_routes: new Set([]),
		matchers: async () => {
			
			return {  };
		},
		server_assets: {}
	}
}
})();

export const prerendered = new Set([]);

export const base = "";