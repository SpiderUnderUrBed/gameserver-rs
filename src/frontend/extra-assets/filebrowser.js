Dropzone.autoDiscover = false;
const basePath = document.querySelector('meta[name="site-url"]').content.replace(/\/$/, '');
let previous_path = '';
let current_path = '';
let globalWs = null;
let uploaderVisible = false;

const modes = [
	{ id: "Copy", displayName: "Copy" },
	{ id: "Move", displayName: "Move" },
	{ id: "Zip", displayName: "Zip" },
	{ id: "Unzip", displayName: "Unzip" },
	{ id: "Download", displayName: "Download" },
	{ id: "DownloadAll", displayName: "Download All" },
	{ id: "UploadAll", displayName: "Upload All" },
	{ id: "None", displayName: "None" },
];

let newSelectedMode = "None"

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

function modeToggle(){
	let modeSelector = document.getElementById('mode-selector');
	if (modeSelector.dataset.mode == "None") {
		newSelectedMode = modes[0].id;
	} else {
		//else if (modeSelector.dataset.mode == "DownloadAll" || modeSelector.dataset.mode == "UploadAll") {
		//} else {
		const currentIndex = modes.findIndex(m => m.id === modeSelector.dataset.mode);
		newSelectedMode = modes[currentIndex + 1].id;
		if (newSelectedMode == "DownloadAll" || newSelectedMode == "UploadAll") {
			let src = document.getElementById('src');
			let dest = document.getElementById('dest');
			// src.disabled = true;
			// dest.disabled = true;
			src.style.color = 'grey';
			dest.style.color = 'grey';
		} else {
			let src = document.getElementById('src');
			let dest = document.getElementById('dest');
			// src.disabled = false;
			// dest.disabled = false;
			src.style.color = '';
			dest.style.color = '';
		}
	}
	modeSelector.dataset.mode = newSelectedMode;
	const selectedMode = modes.find(m => m.id === newSelectedMode);
	//console.log(selectedMode.displayName)
	modeSelector.textContent = `Current mode: ${selectedMode.displayName}`;
	console.log(modeSelector.dataset.mode)
}

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
								if (newSelectedMode == "None") {
									if (filename === '..') {
										const pathParts = path.split('/').filter(p => p !== '');
										pathParts.pop();
										get_files(pathParts.join('/'));
									} else {
										get_files(path ? `${path}/${filename}` : filename);
									}
								} else {
									if (src.textContent != "" && src.textContent != "_"){
										let dest = document.getElementById("dest");
										dest.textContent = `dest: ${filename}`;
										dest.dataset.dest = filename;
									} else {
										src.textContent = `source: ${filename}`;
										src.dataset.src = filename;
									}
								}
							};
						} else {
							button.onclick = () => {
								if (newSelectedMode == "None") {
									open_editor(filename);
								} else {
									let src = document.getElementById("src");
									src.textContent = `source: ${filename}`;
									src.dataset.src = filename;
									// if (src.textContent != "" && src.textContent != "_"){
									// 	let dest = document.getElementById("dest");
									// 	dest.textContent = `dest: ${filename}`;
									// 	dest.dataset.dest = filename;
									// } 
								}
							}
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

async function executeFileOperation(){
// const res = await fetch(`${basePath}/api/getfiles`, {
// 	method: 'POST',
// 	headers: { 'Content-Type': 'application/json' },
// 	body: JSON.stringify({ type: 'command', message: path, authcode: '0' })
// });

//newSelectedMode

	// FileOperations::FileDownloadOperation(_) => "FileDownloadOperation",
	// FileOperations::FileZipOperation(_) => "FileZipOperation",
	// FileOperations::FileMoveOperation(_) => "FileMoveOperation",
	// FileOperations::FileUnzipOperation(_) => "FileUnzipOperation",
	// FileOperations::FileCopyOperation(_) => "FileCopyOperation",

	let src_element = document.getElementById("src");
	let dest_element = document.getElementById("dest");
	let src = src_element.dataset.src
	let dest = dest_element.dataset.dest || current_path || "";
	let final_operation = "";
	if (newSelectedMode == "Move") {
		final_operation = "FileMoveOperation"
	} else if (newSelectedMode == "Copy") {
		final_operation = "FileCopyOperation"
	} else if (newSelectedMode == "Zip"){
		final_operation = "FileZipOperation"
	} else if (newSelectedMode == "DownloadAll"){
		final_operation = "FileDownloadAllOperation"
	} else if (newSelectedMode == "UploadAll"){
		final_operation = "FileUploadAllOperation"
	} else if (newSelectedMode == "Unzip"){
		final_operation = "FileUnzipOperation"
	} else if (newSelectedMode == "Download"){
		await downloadFileSimple(src);
		clearFileOperation();
		return;
	}


	try {
		const res = await fetch(`${basePath}/api/fileoperations`, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ 
				src: {
					kind: final_operation,
					data: src
				},
				dest: {
					kind: final_operation,
					data: dest
				},
				metadata: ""
			})
		});
		src_element.textContent = '_'
		src_element.dataset.src = ''
		dest_element.textContent = '_'
		dest_element.dataset.dest = ''
		get_files('');

	} catch (e){
		console.log(e)
	}
}
// async function downloadFileSimple(filePath) {
// 	if (!filePath) {
// 		alert('No file selected to download');
// 		return;
// 	}

// 	const fullFilePath = current_path ? `${current_path}/${filePath}` : filePath;
	
// 	const a = document.createElement('a');
// 	a.href = `${basePath}/api/download/${encodeURIComponent(fullFilePath)}`;
// 	a.download = filePath;
// 	a.style.display = 'none';
// 	document.body.appendChild(a);
// 	a.click();
// 	document.body.removeChild(a);
// }
async function downloadFileSimple(filePath) {
    console.log('[downloadFileSimple] Starting download...');
    console.log('[downloadFileSimple] Input filePath:', filePath);
    console.log('[downloadFileSimple] current_path:', current_path);
    
    if (!filePath) {
        console.error('[downloadFileSimple] No file path provided');
        alert('No file selected to download');
        return;
    }
    
    const fullFilePath = current_path ? `${current_path}/${filePath}` : filePath;
    console.log('[downloadFileSimple] Full file path:', fullFilePath);
    
    const encodedPath = encodeURIComponent(fullFilePath);
    console.log('[downloadFileSimple] Encoded path:', encodedPath);
    
    const downloadUrl = `${basePath}/api/download/${encodedPath}`;
    console.log('[downloadFileSimple] Download URL:', downloadUrl);
    console.log('[downloadFileSimple] basePath:', basePath);
    
    // Test if the endpoint exists
    try {
        console.log('[downloadFileSimple] Testing endpoint with HEAD request...');
        const testResponse = await fetch(downloadUrl, { method: 'HEAD' });
        console.log('[downloadFileSimple] HEAD response status:', testResponse.status);
        console.log('[downloadFileSimple] HEAD response headers:', 
            Array.from(testResponse.headers.entries()));
        
        if (!testResponse.ok) {
            console.error('[downloadFileSimple] Endpoint returned error:', testResponse.status, testResponse.statusText);
            alert(`Download failed: ${testResponse.status} ${testResponse.statusText}`);
            return;
        }
    } catch (error) {
        console.error('[downloadFileSimple] Failed to test endpoint:', error);
        alert('Failed to connect to download endpoint: ' + error.message);
        return;
    }
    
    console.log('[downloadFileSimple] Creating download link element...');
    const a = document.createElement('a');
    a.href = downloadUrl;
    a.download = filePath;
    a.style.display = 'none';
    
    console.log('[downloadFileSimple] Link properties:', {
        href: a.href,
        download: a.download,
        style: a.style.cssText
    });
    
    document.body.appendChild(a);
    console.log('[downloadFileSimple] Link appended to body');
    
    console.log('[downloadFileSimple] Triggering click...');
    a.click();
    
    // Wait a moment before removing to ensure download starts
    setTimeout(() => {
        document.body.removeChild(a);
        console.log('[downloadFileSimple] Link removed from body');
        console.log('[downloadFileSimple] Download initiated successfully');
    }, 100);
}

function clearFileOperation(){
	let src_element = document.getElementById("src");
	let dest_element = document.getElementById("dest");
	src_element.textContent = '_'
	src_element.dataset.src = ''
	dest_element.textContent = '_'
	dest_element.dataset.dest = ''
}