<script lang="ts">
	import { onMount, setContext } from 'svelte';
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
	import FileOperations from '../../../components/dashboard/FileOperations.svelte';
	import { FileOperationStore } from '../../../lib/stores/fileOperationStore.svelte';

	let hovered: { is_dir: boolean; name: string } | null = $state({
		name: "",
		is_dir: false
	});
	let enabled_size = $state(true);
	let directory_size = $state();
	let show_file_operations = $state(false);
	let timeout = 1000;

	// let checked_item_1: { kind: "Folder" | "File" | string; data: string } | null  = null;
	// let checked_item_2: { kind: "Folder" | "File" | string; data: string } | null  = null;
	let total_checked = $state(0);

	let fileOperationStore = new FileOperationStore();
	setContext('fileOperationStore', fileOperationStore);

	onMount(() => {
		fileBrowserStore.fetchFiles('');
	});

	function navigate(entry: { is_dir: boolean; name: string }) {
		if (!entry) return;
		if (entry.is_dir) {
			if (entry.name === '..') {
				const segments = fileBrowserStore.path.split('/').filter(Boolean);
				segments.pop();
				fileOperationStore.path = segments.join('/');
				fileBrowserStore.fetchFiles(segments.join('/'));
			} else {
				const nextPath = fileBrowserStore.path
					? `${fileBrowserStore.path}/${entry.name}`
					: entry.name;
				fileOperationStore.path = nextPath;
				fileBrowserStore.fetchFiles(nextPath);
			}
		} else {
			fileBrowserStore.fetchFileContent(entry.name);
		}
	}

	async function onUpload(data: { files: FileList }) {
		fileBrowserStore.uploadFiles(data.files);
	}

	function updateCheck(entry: { is_dir: boolean; name: string }, checked: boolean){
		if (checked) {
			total_checked += 1;
			if (fileOperationStore.first_item === null) {
				fileOperationStore.first_item = entry 
			} else {
				fileOperationStore.second_item = entry 
			}
		} else {
			if (fileOperationStore.first_item?.name == entry.name){
				fileOperationStore.first_item = null;
			} 
			if (fileOperationStore.second_item?.name == entry.name) {
				fileOperationStore.second_item = null;
			}
			total_checked -= 1;
		}
		show_file_operations = total_checked >= 1; 
	}

	$effect(() =>{
		console.log(total_checked);
		if (!hovered?.is_dir) return;
		directory_size = "."

		const timers: ReturnType<typeof setTimeout>[] = [];
		let cancelled = false;

		timers.push(setTimeout(() => {
			directory_size = "..";
			timers.push(setTimeout(() => {
				directory_size = "...";
				timers.push(setTimeout(() => {
					if (cancelled) return;
					fileBrowserStore.returnFiles(fileBrowserStore.path ? `${fileBrowserStore.path}/${hovered?.name}`
					: hovered?.name).then((files) => {
						directory_size = files.length;
					});
				}, timeout / 3));
			}, timeout / 3));
		}, timeout / 3));

		return () => {
			cancelled = true;
			return () => timers.forEach(clearTimeout);
		}
	})
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
				onclick={() => navigate({ is_dir: true, name: '..' })}
			>
				<ArrowUpRight class="w-4 h-4 mr-1" /> Up
			</button>
		{/if}
	</div>

	<FileDropzone onupload={onUpload} />
	{#if show_file_operations}
		<FileOperations></FileOperations>
	{/if}

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
						<th class="bg-gray-100 flex gap-2">
							<div class="group">
								<div class="group-hover:hidden">Size</div>
								{#if enabled_size}
									<button onclick={() => enabled_size = false} class="hidden bg-red-100 rounded group-hover:block p-2 btn-error">X</button>
								{:else}
									<button onclick={() => enabled_size = true} class="hidden bg-green-100 rounded group-hover:block p-2 btn border-0">I</button>
								{/if}
							</div>
							<p>Name</p>
						</th>
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
							<tr 
								onmouseenter={() => hovered = {...item}}
								onmouseleave={() => hovered = null}
								>
								<th class="flex h-18 gap-3"><input type="checkbox" class="checkbox" disabled={total_checked >= 2 && !(item?.name == fileOperationStore.first_item?.name || item?.name == fileOperationStore.second_item?.name)} onchange={(e) => updateCheck(item, e.currentTarget.checked)}/> {idx + 1}</th>
								<td>
									<div class="flex items-center">
										<span class="w-8 text-sm shrink-0">
											{#if hovered?.name == item.name && hovered ?.is_dir && enabled_size}{directory_size}{/if}
										</span>
										<button class="btn btn-ghost btn-sm gap-2" onclick={() => navigate(item)}>

											{#if item.is_dir}
												<Folder class="w-4 h-4" />
											{:else}
												<FileText class="w-4 h-4" />
											{/if}
											<span>{item.name}</span>
										</button>
									</div>
								</td>
								<td>{item.is_dir}</td>
								<td>
									{#if item.is_dir}
										<button
											class="btn btn-xs btn-outline"
											onclick={() => fileBrowserStore.fetchFileContent(item.name)}
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
					//value={fileBrowserStore.fileContent}
					bind:value={fileBrowserStore.modifiedFileContent}
				></textarea>
				<button class="btn" onclick={() => {fileBrowserStore.uploadCurrentFile()}}>Save</button>
			</div>
		{/if}
	{/if}
</div>
