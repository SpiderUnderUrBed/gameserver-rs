Dropzone.autoDiscover = false;
const basePath = document.querySelector('meta[name="site-url"]').content.replace(/\/$/, '');
let previous_path = '';
let current_path = '';
let globalWs = null;
let uploaderVisible = false;

const toggleButton = document.getElementById('toggleUploader');
const dropzone = document.getElementById('myDropzone');

toggleButton.addEventListener('click', () => {
	uploaderVisible = !uploaderVisible;
	if (uploaderVisible) {
		dropzone.classList.add('show');
		toggleButton.textContent = 'Hide File Uploader';
	} else {
		dropzone.classList.remove('show');
		toggleButton.textContent = 'Show File Uploader';
	}
});

function connectWebSocket() {
	globalWs = new WebSocket(`${basePath}/api/ws`);
	globalWs.addEventListener('open', () => console.log('WebSocket connected'));
	globalWs.addEventListener('close', (event) => {
		console.log('WebSocket disconnected', event.code, event.reason);
		setTimeout(connectWebSocket, 2000);
	});
	globalWs.addEventListener('error', (err) => console.error('WebSocket error:', err));
}
connectWebSocket();

let loadingChunk = false;
let currentOffset = 0;
let currentFilePath = '';
const CHUNK_SIZE = 1024;
let fileTypeMap = {};

async function load_chunk(path, offset) {
	if (!path || path === '' || fileTypeMap[path] === 'folder') return false;
	if (loadingChunk) return false;
	loadingChunk = true;

	try {
		const fullFilePath = current_path ? `${current_path}/${path}` : path;
		const response = await fetch(`${basePath}/api/getfilescontent`, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({
				file_name: fullFilePath,
				file_chunk_offet: offset.toString(),
				file_chunk_size: CHUNK_SIZE.toString()
			})
		});
		if (!response.ok) {
			loadingChunk = false;
			return false;
		}

		const data = await response.json();
		const fileContent = extractMessage(data);
		if (!fileContent) {
			loadingChunk = false;
			return false;
		}

		const editor = document.getElementById('editor');
		const sentinel = document.getElementById('editor-sentinel');
		const lines = fileContent.split(/\r?\n/);
		lines.forEach(line => {
			const div = document.createElement('div');
			div.textContent = line;
			editor.insertBefore(div, sentinel);
		});

		currentOffset += CHUNK_SIZE;
		loadingChunk = false;
		return lines.length > 0;
	} catch (err) {
		console.error('Error loading chunk:', err);
		loadingChunk = false;
		return false;
	}
}

function extractMessage(obj) {
	if (!obj) return null;
	if (typeof obj === 'string') {
		try { return extractMessage(JSON.parse(obj)); } catch { return obj; }
	}
	if (typeof obj.message === 'string') return extractMessage(obj.message);
	if (obj.data) return extractMessage(obj.data);
	return null;
}

async function open_editor(filename) {
	if (!filename || filename === '' || fileTypeMap[filename] === 'folder') return;
	const editor = document.getElementById('editor');
	editor.querySelectorAll('div:not(#editor-sentinel)').forEach(n => n.remove());
	currentOffset = 0;
	currentFilePath = filename;
	await load_chunk(currentFilePath, currentOffset);
}

document.addEventListener('DOMContentLoaded', () => {
	const editor = document.getElementById('editor');
	const sentinel = document.getElementById('editor-sentinel');
	const observer = new IntersectionObserver((entries) => {
		entries.forEach((entry) => {
			if (entry.isIntersecting) load_chunk(currentFilePath, currentOffset);
		});
	}, { root: editor, threshold: 1.0 });
	observer.observe(sentinel);
});

async function get_files(path) {
	console.log("Getting files for path:", path);
	const fileview = document.getElementById('center');
	if (!path) path = '';
	current_path = path;

	// Show loading spinner immediately before any request
	fileview.innerHTML = '<div style="padding: 20px; text-align: center;"><img src="icons/loading-load.gif" alt="Loading..." style="width: 32px; height: 32px;"><div style="margin-top: 10px; color: #666;">Loading files...</div></div>';

	async function fetchFiles(isRetry = false) {
		console.log("Fetching files, isRetry:", isRetry);
		try {
			const res = await fetch(`${basePath}/api/getfiles`, {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ type: 'command', message: path, authcode: '0' })
			});
			const text = await res.text();
			console.log("API response:", text);
			
			if (res.ok) {
				try {
					const data = JSON.parse(text);
					let filelist = data.list?.data || [];
					console.log("Parsed filelist:", filelist, "Length:", filelist.length);

					if (!isRetry && filelist.length === 0) {
						console.log("Empty file list on first attempt, showing retry message");
						fileview.innerHTML = '<div style="padding: 20px; text-align: center; color: #666;">There might be an issue, will confirm the files and directories in a few seconds...</div>';
						setTimeout(() => {
							console.log("Retrying file fetch...");
							fetchFiles(true);
						}, 5000);
						return;
					}

					fileview.innerHTML = '';
					if (path !== '') filelist.unshift({ kind: 'Folder', data: '..' });

					if (filelist.length === 0 && isRetry) {
						fileview.innerHTML = '<div style="padding: 20px; text-align: center; color: #666;">No files found in this directory.</div>';
						return;
					}

					filelist.forEach(item => {
						const filename = item.data;
						const newelement = document.createElement('div');
						newelement.className = 'file-item';
						const button = document.createElement('button');
						fileTypeMap[filename] = item.kind.toLowerCase() === 'folder' ? 'folder' : 'file';
						if (item.kind.toLowerCase() === 'folder') {
							button.className = 'folder-button';
							button.onclick = () => {
								if (filename === '..') {
									const pathParts = path.split('/').filter(p => p !== '');
									pathParts.pop();
									get_files(pathParts.join('/'));
								} else {
									get_files(path ? `${path}/${filename}` : filename);
								}
							};
						} else {
							button.onclick = () => open_editor(filename);
						}
						button.textContent = filename;
						newelement.appendChild(button);
						fileview.appendChild(newelement);
					});
				} catch (parseError) {
					console.error("JSON parse error:", parseError);
					if (!isRetry) {
						console.log("Parse error on first attempt, showing retry message");
						fileview.innerHTML = '<div style="padding: 20px; text-align: center; color: #666;">There might be an issue, will confirm the files and directories in a few seconds...</div>';
						setTimeout(() => {
							console.log("Retrying file fetch after parse error...");
							fetchFiles(true);
						}, 5000);
					} else {
						fileview.innerHTML = '<div style="padding: 20px; text-align: center; color: #ff6666;">Failed to load file list.</div>';
					}
				}
			} else {
				console.error("HTTP error:", res.status, res.statusText);
				if (!isRetry) {
					console.log("HTTP error on first attempt, showing retry message");
					fileview.innerHTML = '<div style="padding: 20px; text-align: center; color: #666;">There might be an issue, will confirm the files and directories in a few seconds...</div>';
					setTimeout(() => {
						console.log("Retrying file fetch after HTTP error...");
						fetchFiles(true);
					}, 5000);
				} else {
					fileview.innerHTML = '<div style="padding: 20px; text-align: center; color: #ff6666;">Error loading files.</div>';
				}
			}
		} catch (err) {
			console.error("Fetch error:", err);
			if (!isRetry) {
				console.log("Fetch error on first attempt, showing retry message");
				fileview.innerHTML = '<div style="padding: 20px; text-align: center; color: #666;">There might be an issue, will confirm the files and directories in a few seconds...</div>';
				setTimeout(() => {
					console.log("Retrying file fetch after fetch error...");
					fetchFiles(true);
				}, 5000);
			} else {
				fileview.innerHTML = '<div style="padding: 20px; text-align: center; color: #ff6666;">Error loading files.</div>';
			}
		}
	}

	fetchFiles(false);
}
get_files('');

const dz = new Dropzone('#myDropzone', {
	url: `${basePath}/api/upload`,
	paramName: 'file',
	method: 'post',
	maxFilesize: 1024,
	parallelUploads: 10,
	uploadMultiple: false,
	addRemoveLinks: true,
	dictDefaultMessage: 'Drop files or folders here to upload',
	init: function () {
		this.on('success', () => get_files(current_path));
	}
});
dz.hiddenFileInput.setAttribute('webkitdirectory', true);

document.getElementById('saveBtn').addEventListener('click', async () => {
	const editor = document.getElementById('editor');
	const text = Array.from(editor.children)
		.filter(c => c.id !== 'editor-sentinel')
		.map(c => c.textContent)
		.join('\n');

	if (!currentFilePath) {
		alert('No file open to save.');
		return;
	}

	try {
		const fullFilePath = current_path ? `${current_path}/${currentFilePath}` : currentFilePath;

		const blob = new Blob([text], { type: 'text/plain' });
		const formData = new FormData();
		formData.append('file', blob, currentFilePath);

		const res = await fetch(`${basePath}/api/upload`, {
			method: 'POST',
			body: formData
		});

		if (!res.ok) {
			throw new Error(`Upload failed: ${res.status}`);
		}

		console.log(`Saved ${fullFilePath}`);
		get_files(current_path); 
	} catch (err) {
		console.error('Error saving file:', err);
		alert('Failed to save file.');
	}
});
