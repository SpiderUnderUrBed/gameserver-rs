<script lang="ts">
	import { onMount } from "svelte";
	import { httpClient } from "../../../../lib/utils/http";

    let { integration } = $props();

    let playercount = $state();

    let showPlayerCountTooltip = $state(false);

    interface SimpleMessagePayload {
        type: String,
        message: String
    }

    let fetchPlayerCount = async () => {
        //showPlayerCountTooltip = true;
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

    $effect(() => {
        showPlayerCountTooltip;
        fetchPlayerCount()
    })
    onMount(async() => {
        await fetchPlayerCount()
    })
</script>
<div>
    <div class="w-64 tooltip" data-tooltip-target="player-count-tooltip" role="button" tabindex="0" onmouseleave={() => showPlayerCountTooltip = false} onmouseenter={() => showPlayerCountTooltip = true}>Hover for player count</div>
    
    {#if showPlayerCountTooltip}
    <div data-tooltip="player-count-tooltip"
        class="absolute z-50 whitespace-normal break-words rounded-lg bg-black py-1.5 px-3 font-sans text-sm font-normal text-white focus:outline-none">
        Player count is {playercount}
    </div>
    {/if}
</div>