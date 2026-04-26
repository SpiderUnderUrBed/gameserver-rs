import { httpClient } from '../utils/http';

export interface FileEntry {
	kind: 'Folder' | 'File' | string;
	data: string;
}

export class FileBrowserStore {
	public path = $state('');
	public items = $state<FileEntry[]>([]);
	public loading = $state(false);
	public error = $state<string | null>(null);
	public fileContent = $state('');
	public modifiedFileContent = $state('');
	public selectedFile = $state('');

	private async processResponse(res: any) {
		if (res?.list?.data) {
			return res.list.data as FileEntry[];
		}
		return [];
	}

	public async fetchFiles(path: string = this.path) {
		this.loading = true;
		this.error = null;
		this.fileContent = '';
		this.modifiedFileContent = '';
		this.selectedFile = '';
		try {
			const response = await httpClient
				.post('/api/getfiles', {
					json: { type: 'command', message: path, authcode: '0' }
				})
				.json<any>();

			const data = await this.processResponse(response);
			if (path && path !== '') {
				this.items = [{ kind: 'Folder', data: '..' }, ...data];
			} else {
				this.items = data;
			}
			this.path = path;
		} catch (err) {
			this.error = 'Failed to fetch files';
			console.error(err);
		} finally {
			this.loading = false;
		}
	}

	public async fetchFileContent(filename: string) {
		this.loading = true;
		this.error = null;
		this.fileContent = '';
		this.modifiedFileContent = '';
		this.selectedFile = filename;
		try {
			const fullFileName = this.path ? `${this.path}/${filename}` : filename;
			const response = await httpClient
				.post('/api/getfilescontent', {
					json: {
						file_name: fullFileName,
						file_chunk_offet: '0',
						file_chunk_size: '1000000'
					}
				})
				.json<any>();

			const extractMessage = (obj: any): string => {
				if (!obj) return '';
				if (typeof obj === 'string') {
					try {
						return extractMessage(JSON.parse(obj));
					} catch {
						return obj;
					}
				}
				if (typeof obj.message === 'string') return extractMessage(obj.message);
				if (obj.data) return extractMessage(obj.data);
				if (typeof obj === 'object' && obj !== null) {
					if (obj.file_content) return obj.file_content;
					if (obj.content) return obj.content;
				}
				return '';
			};

			this.fileContent = extractMessage(response) ?? '';
			this.modifiedFileContent = extractMessage(response) ?? '';
		} catch (err) {
			this.error = 'Failed to fetch file content';
			console.error(err);
		} finally {
			this.loading = false;
		}
	}
	public async uploadCurrentFile(){
		const fullFileName = this.path ? `${this.path}/${this.selectedFile}` : this.selectedFile;
		try {
			const blob = new Blob([this.modifiedFileContent], { type: 'text/plain' });
			const formData = new FormData();
			formData.append('file', blob, fullFileName);

			const res = await httpClient.post(`/api/upload`, {
				method: 'POST',
				body: formData
			});

			if (!res.ok) {
				throw new Error(`Upload failed: ${res.status}`);
			}

			console.log(`Saved ${fullFileName}`);
		} catch (err) {
			console.error('Error saving file:', err);
			alert('Failed to save file.');
		}

	}

	public async uploadFiles(files: FileList | File[]) {
		if (!files || files.length === 0) return;
		this.loading = true;
		this.error = null;
		try {
			const form = new FormData();
			Array.from(files).forEach((file) => form.append('file', file));

			const response = await fetch(`/api/upload`, {
				method: 'POST',
				body: form
			});

			if (!response.ok) {
				throw new Error(`Upload failed ${response.status}`);
			}
			await this.fetchFiles(this.path);
		} catch (err) {
			this.error = 'Failed to upload file(s)';
			console.error(err);
		} finally {
			this.loading = false;
		}
	}
}

export const fileBrowserStore = new FileBrowserStore();
