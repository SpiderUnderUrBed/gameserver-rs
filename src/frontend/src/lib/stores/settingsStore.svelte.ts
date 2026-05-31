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

export type FilterTypes = "client" | "server" | "any";

export type FileSystemDriver = 
    | { kind: "tcp" }
    | { kind: "none" }

export type Filters = 
    | { kind: "alternating_line" }
    | { kind: "none" }
    | { kind: "terminal" }
    | { kind: "duplicates" }

export type FilterModifyStructure = { name: string } & Filters
export interface GlobalSettings {
    enable_statistics_on_home_page: boolean,
    enable_nodes_on_home_page: boolean,
    console_entry_on_top: boolean,
    force_sandbox: boolean,
    disable_custom_servers: boolean,
    file_system_driver: FileSystemDriver,
    filter: Filters,
    rcon_url: string,
    rcon_password: string
}
export interface EphemeralSettings {
    lock: boolean,
}

export interface UserSettings {
    client_filter: Filters[]
}

export interface Settings extends GlobalSettings, EphemeralSettings, UserSettings {}

export class SettingsStore {
    public currentSettings = $state<Settings>();

    async init(){
        
    }
    private async getGlobalSettings(): Promise<GlobalSettings | undefined> {
        try {
            const response = await httpClient
                .get('/api/getsettings')
                .json<GlobalSettings>();
            return response;
        } catch (e) {
            console.error(e);
        }
    }

    public async refreshSettings() {
        await this.getSettings().then((settings) => {
            this.currentSettings = { ...this.currentSettings, ...settings } as Settings;
            this.loadUserSettings(); 
        })
    }

    public async changeSettings(settings: Settings){
        this.currentSettings = settings;
    }

    private toGlobalSettings({ lock, client_filter, ...global }: Settings): GlobalSettings {
        return global;
    }



    public async getSettings(): Promise<Settings | undefined> {
        return this.getGlobalSettings().then((global_settings) => {
            return { ...this.currentSettings, ...global_settings } as Settings;
        });
    }
    private loadUserSettings() {
        if (this.currentSettings != null) {
            const stored = localStorage.getItem('client_filter');
            this.currentSettings.client_filter = stored ? JSON.parse(stored) : [{ kind: "none" }];
        }
    }
    private storeUserSettings(){
        localStorage.setItem('client_filter', JSON.stringify(this.currentSettings?.client_filter));
    }
    public async syncSettings(){
        if (this.currentSettings) {
            try {
                this.storeUserSettings();
                let global_settings = this.toGlobalSettings(this.currentSettings);
                const response = await httpClient
                    .post('/api/setsettings', {
                        json: {
                            message: {
                                ...global_settings
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
    public filterType(filter: Filters): FilterTypes {
        if (filter.kind != "terminal" && filter.kind != "duplicates"){
            return "server"
        } else {
            return "client"
        }
    }
}