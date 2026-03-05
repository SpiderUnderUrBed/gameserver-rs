const server_div = document.querySelector(".servers");
const basePath = document.querySelector('meta[name="site-url"]').content.replace(/\/$/, '');
    

async function fetchServers() {
    try {
        const response = await fetch(`${basePath}/api/servers`);
        if (response.ok) {
            const data = await response;
            const json_data = await  data.json();
            const servers = json_data.list.data;

            server_div.innerHTML = ""; 

            servers.forEach((server, index) => {
                const button = document.createElement("div");
                button.role = "button";
                button.innerHTML = `<div style="width: 20px"></div><h5>${server.servername}</h5>`;
                button.className = "servers-element";
                button.onclick = () => alert(`Server clicked: ${server.servername}`);
                // const spacer = document.createElement("span");
                // spacer.style = "width: 20px";
                server_div.appendChild(button);
                // server_div.appendChild(spacer);
            });
        } else {
            console.log("Failed to get servers from the server");
            document.getElementById('message').innerText = 'Failed to get servers from the server.';
        }
    } catch (error) {
        document.getElementById('message').innerText = 'Error connecting to the server.';
        console.error('Error fetching servers:', error);
    }
}
fetchServers()

async function addServer(){
    event.preventDefault()
    console.log("adding server");
    
    const server = document.getElementById('create-servername').value;
    const password = document.getElementById('password').value;
    const authcode = "0";

    console.log(server);

    try {
        const response = await fetch(`${basePath}/api/createserver`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json'
        },
        body: JSON.stringify({ server, password, authcode })
        });

        if (!response.ok) {
        const error = await response.text();
        console.error('Server error:', error);
        alert('Failed to create server.');
        } else {
        const result = await response.text();
        console.log('Server created:', result);
        // fetchServers()
        alert('Server created successfully!');
        fetchServers()
        }
    } catch (err) {
        console.error('Request failed:', err);
        alert('An error occurred while creating the server.');
    }
}
async function deleteServer(){
    event.preventDefault()
    
    const server = document.getElementById('delete-servername').value;
    const authcode = "0";

    try {
        const response = await fetch(`${basePath}/api/deleteserver`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json'
        },
        body: JSON.stringify({ server, authcode })
        });

        if (!response.ok) {
        const error = await response.text();
        console.error('Server error:', error);
        alert('Failed to delete server.');
        } else {
        const result = await response.text();
        console.log('Server deleted:', result);
        alert('Server delete successfully!');
        fetchServers()
        }
    } catch (err) {
        console.error('Request failed:', err);
        alert('An error occurred while creating the server.');
    }
}