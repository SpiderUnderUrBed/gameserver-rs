// const basePath = document.querySelector('meta[name="site-url"]').content.replace(/\/$/, '');

// async function toggleIntergration(button){
// }

// async function toggleIntergrationOnHomePage(button){
//     let isEnabled = ""
// }
const basePath = document.querySelector('meta[name="site-url"]').content.replace(/\/$/, '');
// prefix all /api/* calls with basePath, e.g basePath + "/api/*"

// data Intergration = Intergration
//   { name :: String
//   , status :: String
//   , type_ :: String
//   } deriving (Show, Generic, Eq)

// for /api/createintergration (enabling) do this

//IGNORE UNDER ME
//~~for /api/deleteintergration (disabling), just send the property 'intergration' with the name of the intergration to delete
// e.g minecraft~~

// /api/deleteintergration (disabling), should just send a string with the name of the intergration in the data portion of the request


async function toggleIntergration(button){

    const integration = {
        element: {
            kind: "Intergration",
            data: {
                status: "enabled",
                type: "minecraft",
                settings: {}
            }
        },
        jwt: "",
        require_auth: false
    };

    fetch(`${basePath}/api/createintergrations`, {
        method: "POST",
        headers: {
            "Content-Type": "application/json"
        },
        body: JSON.stringify(integration)
    })
    .then(async response => {
        if (!response.ok) {
            const body = await response.text();

            throw new Error(`Request failed (${response.status} ${response.statusText}): ${body}`);
        }

        return response.json();
    })
    .then(data => {
        fetchIntergrationColors()
        console.log("Integration enabled:", data);
        // optionally update UI here using `button`
    })
    .catch(error => {
        console.error("Error enabling integration:", error);
    });
}
    

async function toggleIntergrationOnHomePage(button) {
    let isEnabled = "";
    let settings = {};
    
    const response = await fetch(`${basePath}/api/intergrations`);
    const data = await response.json();
    
    const intergrations = data.list?.data || [];
    
    intergrations.forEach(integration => {
        if (integration.type === "minecraft") {
            settings = integration.settings;
            if (integration.status === "enabled") {
                isEnabled = "disabled";
                console.log("Toggling to: Disabled");
            } else if (integration.status === "disabled") {
                isEnabled = "enabled";
                console.log("Toggling to: Enabled");
            } 
            // else {
            //     isEnabled = "enabled";
            //     console.log("Status empty, toggling to: Enabled");
            // }
        } 
    });

    settings = {
        "enable_test": !settings.enable_test,
        "_enable_test_hook": {
            "kind": "MinecraftEnableRcon",
            //"data": {}
        }
    }
    console.log(settings)
    
    if (!isEnabled) {
        console.error("Could not determine status - minecraft integration not found!");
        return;
    }
    
    const integration = {
        element: {
            kind: "Intergration",
            data: {
                status: isEnabled,
                type: "minecraft",
                settings
            }
        },
        jwt: "",
        require_auth: false
    };
    console.log(integration)
    
    fetch(`${basePath}/api/modifyintergrations`, {
        method: "POST",
        headers: {
            "Content-Type": "application/json"
        },
        body: JSON.stringify(integration)
    })
    .then(async response => {
        if (!response.ok) {
            const body = await response.text();
            throw new Error(`Request failed (${response.status} ${response.statusText}): ${body}`);
        }
        return response.json();
    })
    .then(data => {
        fetchIntergrationColors();
    })
    .catch(error => {
        console.error("Error enabling integration on home page:", error);
    });
}

async function fetchIntergrationColors() {
    const basePath = document.querySelector('meta[name="site-url"]').content.replace(/\/$/, '');

    try {
        const response = await fetch(`${basePath}/api/intergrations`);
        //console.log("Raw response:", response);

        const data = await response.json();
        console.log("Fetched JSON:", data);

        // Handle both array and object responses
        //const intergrations = Array.isArray(data) ? data : data.intergrations || [];
        const all_intergrations = document.querySelectorAll('[data-intergration]');
        all_intergrations.forEach(element_integration => {
            element_integration.backgroundColor = "red"
        })

        const intergrations = data.list?.data || [];

        intergrations.forEach(integration => {
            //console.log("AHHHH")
            const found_intergration = document.querySelector(`[data-integration="${integration.type}"]`);
            if (found_intergration) {
                const inner_intergration = found_intergration.querySelector('#top-intergration-buttons');
                let inner_button = inner_intergration.querySelector(`#enable-intergration-button`);
                inner_button.style.backgroundColor = "green"
                let inner_home_button = inner_intergration.querySelector(`#enable-intergration-on-home-button`);
                if (integration.status == "enabled"){
                    inner_home_button.style.backgroundColor = "green"
                } else {
                    inner_home_button.style.backgroundColor = "red"
                }
                    // integration.status === "enabled" ? "green" : "grey";
            }
        });
        //enable-intergration-on-home-button

    } catch (error) {
        console.error("Failed to fetch intergrations:", error);
    }
}

fetchIntergrationColors();