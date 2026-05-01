// pub(crate) enable_statistics_on_home_page: bool,
// pub(crate) enable_nodes_on_home_page: bool,

import { message } from "valibot";
import { httpClient } from "../utils/http";

export interface Settings {
    enable_statistics_on_home_page: boolean,
    enable_nodes_on_home_page: boolean
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