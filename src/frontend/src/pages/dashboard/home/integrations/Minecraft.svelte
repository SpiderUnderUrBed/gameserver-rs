<script lang="ts">
	import { onMount } from "svelte";
	import { httpClient } from "../../../../lib/utils/http";

    let { integration } = $props();

    let playercount = $state();
    
    interface SimpleMessagePayload {
        type: String,
        message: String
    }

    let fetchPlayerCount = async () => {
        try {
            const response = await httpClient.post<SimpleMessagePayload>(`/api/rconcommand`, {
                json: {
                    message: "list",
                    type: "",
                    authcode: ""
                }
            })
            if (response.ok){
                let json = await response.json();
                //console.log(json.message);
                playercount = json.message.slice("There are ".length).split("of")[0].trim();
                //json.message.split(":")[1].split(" ").length;
                //console.log(playercount);
            }
      } catch (e) {
        console.error(e);
      }
    }

    onMount(async() => {
        await fetchPlayerCount()
    })
</script>
<div>
    <button onclick={fetchPlayerCount}>Player count is {playercount}</button>
</div>