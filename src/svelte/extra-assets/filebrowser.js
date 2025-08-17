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
	if (!path || path === '' || fileTypeMap[path] === 'folder') {
		console.warn('[load_chunk] Invalid path or directory, skipping:', path);
		return false;
	}

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
			console.error('Error fetching file:', response.status, response.statusText);
			loadingChunk = false;
			return false;
		}

		const data = await response.json();
		console.log('[load_chunk] Raw chunk response:', data);

		if (!data || !data.message) {
			console.warn('Empty or invalid file content', data);
			loadingChunk = false;
			return false;
		}

		let fileContent;
		fileContent = extractMessage(data);

		if (!fileContent) {
			console.warn('File content is empty or undefined', data);
			loadingChunk = false;
			return false;
		}

		const editor = document.getElementById('editor');
		if (!editor) {
			console.error('Editor element not found');
			loadingChunk = false;
			return false;
		}

		const lines = fileContent.split(/\r?\n/);
		for (let line of lines) {
			const lineEl = document.createElement('div');
			lineEl.textContent = line;
			lineEl.style.fontFamily = 'monospace';
			lineEl.style.whiteSpace = 'pre-wrap';
			editor.appendChild(lineEl);
		}

		currentOffset += CHUNK_SIZE;
		loadingChunk = false;
		return lines.length > 0;
	} catch (err) {
		console.error('Error loading chunk:', err);
		loadingChunk = false;
		return false;
	}
}
let fileContent = null;

function extractMessage(obj) {
	if (!obj) return null;

	if (typeof obj === 'string') {
		try {
			const parsed = JSON.parse(obj);
			return extractMessage(parsed);
		} catch {
			return obj;
		}
	}

	if (typeof obj.message === 'string') {
		return extractMessage(obj.message);
	}

	if (obj.data) {
		return extractMessage(obj.data);
	}

	return null;
}

async function open_editor(filename) {
	if (!filename || filename === '' || fileTypeMap[filename] === 'folder') {
		console.warn('[open_editor] Attempted to open invalid file path or directory:', filename);
		return;
	}

	const editor = document.getElementById('editor');
	if (!editor) return;

	editor.innerHTML = '';
	currentOffset = 0;
	currentFilePath = filename;

	const more = await load_chunk(currentFilePath, currentOffset);
	if (!more) console.warn('[open_editor] No initial content loaded for', currentFilePath);
}

document.addEventListener('DOMContentLoaded', () => {
	const sentinel = document.getElementById('editor-sentinel');
	if (!sentinel) {
		console.error('Sentinel element not found!');
		return;
	}

	const observer = new IntersectionObserver(
		(entries) => {
			entries.forEach((entry) => {
				if (entry.isIntersecting) {
					load_chunk(currentFilePath, currentOffset);
				}
			});
		},
		{
			root: document.querySelector('#file-editor'),
			threshold: 1.0
		}
	);

	observer.observe(sentinel);
});

async function get_files(path) {
	let fileview = document.getElementById('center');
	fileview.innerHTML = '';

	if (!path) path = '';

	console.log('[get_files] Requesting path:', path);
	current_path = path;

	try {
		const res = await fetch(`${basePath}/api/getfiles`, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ type: 'command', message: path, authcode: '0' })
		});
		const text = await res.text();
		console.log('[get_files] raw response:', text);

		if (res.ok) {
			try {
				const data = JSON.parse(text);
				let filelist = data.list.data;

				if (path !== '') {
					filelist.unshift({ kind: 'Folder', data: '..' });
				}

				filelist.forEach((item) => {
					let filename = item.data;
					let newelement = document.createElement('div');
					newelement.className = 'file-item';
					let button = document.createElement('button');

					fileTypeMap[filename] = item.kind.toLowerCase() === 'folder' ? 'folder' : 'file';

					if (item.kind.toLowerCase() === 'folder') {
						button.className = 'folder-button';
						button.onclick = () => {
							if (filename === '..') {
								const pathParts = path.split('/').filter((p) => p !== '');
								pathParts.pop();
								const parentPath = pathParts.join('/');
								previous_path = parentPath;
								get_files(parentPath);
							} else {
								const newPath = path ? `${path}/${filename}` : filename;
								previous_path = path;
								get_files(newPath);
							}
						};
					} else {
						button.onclick = () => open_editor(filename);
					}

					newelement.appendChild(button);
					button.textContent = filename;
					fileview.appendChild(newelement);
				});
			} catch (err) {
				console.warn('[get_files] Invalid JSON response:', err, text);
			}
		} else {
			console.warn(`[get_files] Failed (${res.status}): ${text}`);
		}
	} catch (err) {
		console.error('[get_files] Error fetching files:', err);
	}
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
		this.on('success', (file, response) => {
			console.log('Uploaded:', file.name, response);
			get_files(current_path);
		});
		this.on('error', (file, err) => console.error('Upload error:', err));
	}
});
dz.hiddenFileInput.setAttribute('webkitdirectory', true);
