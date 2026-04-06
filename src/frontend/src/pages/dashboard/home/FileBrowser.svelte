<script lang="ts">
	import { onMount } from 'svelte';
	import { fileBrowserStore } from '../../../lib/stores/fileBrowserStore.svelte';
	import FileDropzone from '../../../components/dashboard/FileDropzone.svelte';
	import {
		Folder,
		FileText,
		Loader2,
		ArrowUpRight,
		RefreshCcw,
		Loader2Icon,
		LoaderCircleIcon
	} from '@lucide/svelte';

	onMount(() => {
		fileBrowserStore.fetchFiles('');
	});

	function navigate(entry: { kind: string; data: string }) {
		if (!entry) return;
		if (entry.kind === 'Folder') {
			if (entry.data === '..') {
				const segments = fileBrowserStore.path.split('/').filter(Boolean);
				segments.pop();
				fileBrowserStore.fetchFiles(segments.join('/'));
			} else {
				const nextPath = fileBrowserStore.path
					? `${fileBrowserStore.path}/${entry.data}`
					: entry.data;
				fileBrowserStore.fetchFiles(nextPath);
			}
		} else {
			fileBrowserStore.fetchFileContent(entry.data);
		}
	}

	async function onUpload(data: { files: FileList }) {
		fileBrowserStore.uploadFiles(data.files);
	}
</script>

<div class="flex flex-col gap-4">
	<h2 class="text-2xl font-bold">File Browser</h2>
	<div class="flex gap-4 flex-wrap">
		<button
			class="btn btn-sm btn-secondary"
			onclick={() => fileBrowserStore.fetchFiles(fileBrowserStore.path)}
		>
			<RefreshCcw class="w-4 h-4 mr-2" /> Refresh
		</button>
		{#if fileBrowserStore.path}
			<button
				class="btn btn-sm btn-outline"
				onclick={() => navigate({ kind: 'Folder', data: '..' })}
			>
				<ArrowUpRight class="w-4 h-4 mr-1" /> Up
			</button>
		{/if}
	</div>

	<FileDropzone onupload={onUpload} />

	{#if fileBrowserStore.loading}
		<div class="flex items-center gap-2 text-base-content/80 justify-center p-8">
			<LoaderCircleIcon class="w-5 h-5 animate-spin" />
			<span>Loading files...</span>
		</div>
	{:else if fileBrowserStore.error}
		<p class="text-error">{fileBrowserStore.error}</p>
	{:else}
		<div class="overflow-x-auto rounded-lg border border-base-300 bg-base-100">
			<table class="table table-zebra table-pin-rows table-pin-cols w-full">
				<thead>
					<tr>
						<th class="bg-gray-100">#</th>
						<th class="bg-gray-100">Name</th>
						<th class="bg-gray-100">Type</th>
						<th class="bg-gray-100">Actions</th>
					</tr>
				</thead>
				<tbody>
					{#if fileBrowserStore.items.length === 0}
						<tr>
							<td colspan="4" class="text-center">No files or directories found</td>
						</tr>
					{:else}
						{#each fileBrowserStore.items as item, idx}
							<tr>
								<th>{idx + 1}</th>
								<td>
									<button class="btn btn-ghost btn-sm gap-2" onclick={() => navigate(item)}>
										{#if item.kind === 'Folder'}
											<Folder class="w-4 h-4" />
										{:else}
											<FileText class="w-4 h-4" />
										{/if}
										<span>{item.data}</span>
									</button>
								</td>
								<td>{item.kind}</td>
								<td>
									{#if item.kind !== 'Folder'}
										<button
											class="btn btn-xs btn-outline"
											onclick={() => fileBrowserStore.fetchFileContent(item.data)}
										>
											Open
										</button>
									{/if}
								</td>
							</tr>
						{/each}
					{/if}
				</tbody>
			</table>
		</div>
		{#if fileBrowserStore.selectedFile}
			<div class="mt-4">
				<h3 class="font-semibold">Editing: {fileBrowserStore.selectedFile}</h3>
				<textarea
					class="textarea textarea-bordered w-full h-64 mt-2 font-mono text-sm"
					readonly
					value={fileBrowserStore.fileContent}
				></textarea>
			</div>
		{/if}
	{/if}
</div>
