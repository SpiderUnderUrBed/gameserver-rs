// pub(crate) enable_statistics_on_home_page: bool,
// pub(crate) enable_nodes_on_home_page: bool,

import { message } from "valibot";
import { httpClient } from "../utils/http";

    // pub(crate) toggled_default_buttons: bool,
    // pub(crate) status_type: String,
    // pub(crate) enabled_rcon: bool,
    // pub(crate) rcon_url: String,
    // pub(crate) rcon_password: String,
    // //pub(crate) driver: String,
    // pub(crate) filter: Filters,
    // pub(crate) file_system_driver: FileSystemDrivers,
    // pub(crate) enable_statistics_on_home_page: bool,
    // pub(crate) enable_nodes_on_home_page: bool,
    // pub(crate) console_entry_on_top: bool,
    // #[cfg_attr(any(feature = "full-stack", feature = "database"), sqlx(json))]
    // pub(crate) current_server: Server,


export type FileSystemDriver = 
    | { kind: "tcp" }
    | { kind: "none" }

export type Filters = 
    | { kind: "alternating_line" }
    | { kind: "none" }

export type FilterModifyStructure = { name: string } & Filters
export interface Settings {
    enable_statistics_on_home_page: boolean,
    enable_nodes_on_home_page: boolean,
    console_entry_on_top: boolean,
    file_system_driver: FileSystemDriver,
    filter: Filters,
    rcon_url: string,
    rcon_password: string
}


export class SettingsStore {
    public currentSettings = $state<Settings | undefined>(undefined)

    async init(){
        
    }

    public async refreshSettings() {
        await this.getSettings().then((settings) => {
            this.currentSettings = settings;
        })
    }

    public async changeSettings(settings: Settings){
        this.currentSettings = settings;

        
    }
    public async getSettings(): Promise<Settings | undefined> {
        try {
            const response = await httpClient
                .get('/api/getsettings')
                .json<Settings>();
            return response;
        } catch (e) {
            console.error(e);
        }
    }
    public async syncSettings(){
        if (this.currentSettings) {
            try {
                const response = await httpClient
                    .post('/api/setsettings', {
                        json: {
                            message: {
                                ...this.currentSettings
                            },
                            type: "",
                            authcode: ""
                        }
                    });
                if (response.ok){
                    console.log("response is ok");
                    this.refreshSettings();
                }
                
            } catch (err) {
                console.error(err);
            } 
        }
    }
}