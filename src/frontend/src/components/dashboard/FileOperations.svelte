<script lang="ts">
	import { getContext } from "svelte";
    import { writable } from "svelte/store";
	import type { FileOperation, FileOperationStore } from "../../lib/stores/fileOperationStore.svelte";

    const fileOperationStore = getContext<FileOperationStore>("fileOperationStore")

    const fileOperation = $derived(fileOperationStore.current_file_operation?.name ?? "None");
    const firstFile = $derived(JSON.stringify(fileOperationStore?.first_item?.data));
    const secondFile = $derived(JSON.stringify(fileOperationStore?.second_item?.data));


</script>
<div class="bg-base-100 rounded p-4">
    <div class="flex justify-center gap-4">
        <button class="bg-black rounded p-4 cursor-pointer" onclick={() => fileOperationStore?.nextMode()}>Current file operation: {fileOperation}</button>
        <div class="bg-black rounded p-4">First file: {firstFile}</div>
        <div class="bg-black rounded p-4">Second file: {secondFile}</div>
        <button class="bg-black rounded p-4 cursor-pointer" onclick={() => fileOperationStore?.executeFileOperation()}>Execute file operation</button>
        <button class="bg-black rounded p-4 cursor-pointer">Clear</button>
    </div>
</div>