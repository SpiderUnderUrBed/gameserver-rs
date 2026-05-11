import { writable } from "svelte/store";
import type { FileEntry } from "./fileBrowserStore.svelte";
import { httpClient } from "../utils/http";
import { metadata } from "valibot";

export interface FileOperation {
    name: String,
    id: String
}


// export const current_file_operation = writable<FileEntry | null>(null);

export class FileOperationStore {

    public current_file_operation = $state<FileOperation | null>(null)
    // public current_file_operation_index: Number = $derived(() => {
    //     this.modes.findIndex((mode) => mode.id == this.current_file_operation?.id)
    // })
    public path = $state<string>("server");
    public first_item = $state<FileEntry | null>(null)
    public second_item = $state<FileEntry | null>(null)
    public modes: FileOperation[] = [
        { id: "Unknown", name: "None" },
        { id: "FileCopyOperation", name: "Copy" },
        { id: "FileMoveOperation", name: "Move" },
        { id: "FileZipOperation", name: "Zip" },
        { id: "FileUnzipOperation", name: "Unzip" },
        { id: "FileDownloadOperation", name: "Download" },
        // { id: "DownloadAll", name: "Download All" },
        // { id: "UploadAll", name: "Upload All" },
    ];
    // public current_file_operation_index = $derived(
    //     this.modes.findIndex(
    //         (mode) => mode.id === this.current_file_operation?.id
    //     )
    // );
	public async executeFileOperation() {
		let final_operation: String | undefined = undefined;
        if (this.current_file_operation?.id == "Download") {
            if (this.first_item?.data){
                await this.downloadFileSimple(this.first_item?.data);
                this.clearFileOperation();
            }
            return;
        } else {
            final_operation = this.current_file_operation?.id
        }
        if (final_operation){
            let request = {
                src: {
                    kind: "FileOperations",
                    data: {
                        kind: final_operation,
                        data: this.first_item ? this.first_item.data : ""
                    }
                },
                dest: {
                    kind: "FileOperations",
                    data: {
                        kind: final_operation,
                        data: this.second_item ? this.second_item.data : ""
                    }
                },
                metadata: ""
            }
            console.log(request);
            const response = await httpClient.post("/api/fileoperations", {
                json: request
            });
            if (!response.ok){
                console.error("File operation failed with " + response.ok)
            }
            this.clearFileOperation();
        }
	}
    public async clearFileOperation(){
        this.first_item = null;
        this.second_item = null;
        this.current_file_operation = this.modes[0];
    }
    public async downloadFileSimple(filePath: String){
            const fullFilePath = this.first_item ? `${this.path}/${filePath}` : this.path;
            if (!fullFilePath) return;
            //const encodedPath = encodeURIComponent(fullFilePath);

            const downloadUrl = `/api/download/${fullFilePath}`;
            window.open(downloadUrl);
            // const response = await httpClient.get(downloadUrl, {});
            // console.log(response);
    }
    public async nextMode(){
        let current_file_operation_index = this.modes.findIndex(
            (mode) => mode.id === this.current_file_operation?.id
        );
        if (current_file_operation_index == this.modes.length){
            this.current_file_operation = this.modes[0];
        } else {
            this.current_file_operation = this.modes[current_file_operation_index+1]
        }
    }
}