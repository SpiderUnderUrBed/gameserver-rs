const nodes_div = document.querySelector(".nodes");
const basePath = document.querySelector('meta[name="site-url"]').content.replace(/\/$/, '');
    
async function fetchNodes() {
    try {
        const response = await fetch(`${basePath}/api/nodes`);
        nodes_div.innerHTML = ""; 

        if (response.ok) {
            const data = await response.json();
            console.log(data);
            const nodes = data.list.data;

            nodes.forEach((node, index) => {
                const button = document.createElement("button");
                button.textContent = node;
                button.className = "nodes-element";
                button.onclick = () => alert(`Node clicked: ${node}`);
                nodes_div.appendChild(button);
            });
        } else {
            document.getElementById('message').innerText = 'Failed to get nodes from the server.';
        }
    } catch (error) {
        document.getElementById('message').innerText = 'Error connecting to the server.';
        console.log('Error fetching nodes:', error);
    }
}
fetchNodes()
// async function fetchNodes() {
//     try {
//         const response = await fetch(`${basePath}/api/nodes`);
//         if (response.ok) {
//             const data = await response;
//             console.log(data);  
//             const json_data = await  data.json();
//             const nodes = json_data.list.data;

//             node_div.innerHTML = ""; 

//             nodes.forEach((node, index) => {
//                 const button = document.createElement("div");
//                 button.role = "button";
//                 button.innerHTML = `<div style="width: 20px"></div><h5>${node.nodename}</h5>`;
//                 button.className = "nodes-element";
//                 button.onclick = () => alert(`Node clicked: ${node.nodename}`);
//                 // const spacer = document.createElement("span");
//                 // spacer.style = "width: 20px";
//                 node_div.appendChild(button);
//                 // node_div.appendChild(spacer);
//             });
//         } else {
//             console.log("Failed to get nodes from the server");
//             document.getElementById('message').innerText = 'Failed to get nodes from the server.';
//         }
//     } catch (error) {
//         document.getElementById('message').innerText = 'Error connecting to the server.';
//         console.error('Error fetching nodes:', error);
//     }
// }
// fetchNodes()

async function addNode(){
    event.preventDefault()
    console.log("adding node");
    
    const node = document.getElementById('create-nodename').value;
    const password = document.getElementById('nodeip').value;
    const authcode = "0";

    console.log(node);

    try {
        const response = await fetch(`${basePath}/api/addnode`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json'
        },
        body: JSON.stringify({ node, password, authcode })
        });

        if (!response.ok) {
        const error = await response.text();
        console.error('Server error:', error);
        alert('Failed to create node.');
        } else {
        const result = await response.text();
        console.log('Node created:', result);
        // fetchNodes()
        alert('Node created successfully!');
        fetchNodes()
        }
    } catch (err) {
        console.error('Request failed:', err);
        alert('An error occurred while creating the node.');
    }
}
async function deleteNode(){
    event.preventDefault()
    
    const node = document.getElementById('delete-nodename').value;
    const authcode = "0";

    try {
        const response = await fetch(`${basePath}/api/deletenode`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json'
        },
        body: JSON.stringify({ node, authcode })
        });

        if (!response.ok) {
        const error = await response.text();
        console.error('Server error:', error);
        alert('Failed to delete node.');
        } else {
        const result = await response.text();
        console.log('Node deleted:', result);
        alert('Node delete successfully!');
        fetchNodes()
        }
    } catch (err) {
        console.error('Request failed:', err);
        alert('An error occurred while creating the node.');
    }
}