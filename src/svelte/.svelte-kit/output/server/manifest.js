export const manifest = (() => {
function __memo(fn) {
	let value;
	return () => value ??= (value = fn());
}

return {
	appDir: "_app",
	appPath: "_app",
	assets: new Set(["favicon.png","index.html.old","main.css","main.html","main.js","users.html.old","users.js.old"]),
	mimeTypes: {".png":"image/png",".css":"text/css",".html":"text/html",".js":"text/javascript"},
	_: {
		client: {start:"_app/immutable/entry/start.DOU7MhmX.js",app:"_app/immutable/entry/app.CKCb-enY.js",imports:["_app/immutable/entry/start.DOU7MhmX.js","_app/immutable/chunks/DEDaQRwP.js","_app/immutable/chunks/BO0ZvBGM.js","_app/immutable/chunks/CvbJCj2Z.js","_app/immutable/entry/app.CKCb-enY.js","_app/immutable/chunks/BO0ZvBGM.js","_app/immutable/chunks/D47ybobq.js","_app/immutable/chunks/Bz9HKDcm.js","_app/immutable/chunks/CNE7uOga.js","_app/immutable/chunks/CvbJCj2Z.js"],stylesheets:[],fonts:[],uses_env_dynamic_public:false},
		nodes: [
			__memo(() => import('./nodes/0.js')),
			__memo(() => import('./nodes/1.js'))
		],
		routes: [
			
		],
		prerendered_routes: new Set(["/"]),
		matchers: async () => {
			
			return {  };
		},
		server_assets: {}
	}
}
})();
