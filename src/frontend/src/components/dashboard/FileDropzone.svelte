<script lang="ts">
	import { createEventDispatcher } from 'svelte';
	import { CloudUploadIcon, UploadCloud, UploadCloudIcon } from '@lucide/svelte';

	const dispatch = createEventDispatcher();
	let highlight = $state(false);

	function onDragOver(event: DragEvent) {
		event.preventDefault();
		highlight = true;
	}

	function onDragLeave() {
		highlight = false;
	}

	async function onDrop(event: DragEvent) {
		event.preventDefault();
		highlight = false;

		const files = event.dataTransfer?.files;
		if (files && files.length > 0) {
			dispatch('upload', { files });
		}
	}

	function onFileSelect(event: Event) {
		const input = event.target as HTMLInputElement;
		if (!input.files) return;
		dispatch('upload', { files: input.files });
		input.value = '';
	}
</script>

<div
	role="button"
	tabindex="0"
	aria-label="File upload dropzone"
	class="border-2 rounded-lg p-4 transition-all text-center cursor-pointer"
	class:border-blue-500={highlight}
	class:border-dashed={true}
	class:bg-blue-50={highlight}
	ondragover={onDragOver}
	ondragenter={onDragOver}
	ondragleave={onDragLeave}
	ondrop={onDrop}
>
	<CloudUploadIcon class="inline-block w-8 h-8 mb-2" />
	<p>Drag and drop files here, or click to select</p>
	<input type="file" multiple class="hidden" id="fileyear" onchange={onFileSelect} />
	<label for="fileyear" class="btn btn-sm btn-primary mt-2">Choose files</label>
</div>
