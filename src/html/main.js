const nodes_div = document.querySelector(".nodes");
const container = document.getElementById("container");
const basePath = document.querySelector('meta[name="site-url"]').content.replace(/\/$/, '');

const consoleInput = document.querySelector(".console-input");
const historyContainer = document.querySelector(".console-history");

async function addResult(inputAsString, output, addInput, addOutput){
    const outputAsString = output instanceof Array ? `[${output.join(",")}]`: output.toString();
    console.log(inputAsString, outputAsString);

    if (addInput) {
        const inputLogElement = document.createElement("div");
        inputLogElement.classList.add("console-input-log");
        inputLogElement.textContent = `> ${inputAsString}`;
        historyContainer.append(inputLogElement)
    }
    if (addOutput){
        const outputLogElement = document.createElement("div");
        outputLogElement.classList.add("console-output-log");
        outputLogElement.textContent = outputAsString;
        historyContainer.append(outputLogElement);
    }
}

async function websocket() {
    const ws = new WebSocket(`${basePath}/ws`);

    ws.addEventListener("open", () => {
        console.log("We are connected");

        ws.send("Hey, how is it going?");
    });

    ws.addEventListener("message", e => {
        console.log(e.data);
        addResult("", e.data, false, true);
    });
}
websocket()

consoleInput.addEventListener("keyup", e => {
    const code = consoleInput.value.trim();
    if (code.length === 0){
        return;
    }
    if (e.key === "Enter"){
        try {
            addResult(code, "test", true, true);
            //addResult(code, eval(code));
        } catch (err) {
            addResult(code, err, true, true);
        }

        consoleInput.value = "";
        historyContainer.scrollTop = historyContainer.scrollHeight;
    }
});

async function fetchNodes() {
    try {
        const response = await fetch(`${basePath}/api/nodes`);
        if (response.ok) {
            const data = await response.json();
            const nodes = data.list;

            nodes_div.innerHTML = ""; 

            nodes.forEach((node, index) => {
                const button = document.createElement("button");
                button.textContent = node;
                button.className = "nodes-element";
                button.onclick = () => alert(`Node clicked: ${node}`);
                nodes_div.appendChild(button);
            });

            // Log all node buttons
            console.log("All nodes:");
            document.querySelectorAll(".nodes button").forEach((btn, index) => {
                console.log(`Node ${index + 1}:`, btn.textContent);
            });
        } else {
            document.getElementById('message').innerText = 'Failed to get nodes from the server.';
        }
    } catch (error) {
        document.getElementById('message').innerText = 'Error connecting to the server.';
        console.error('Error fetching nodes:', error);
    }
}
fetchNodes()

async function createDefaultServer() {
    const message = "create_server";

    try {
        const response = await fetch(`${basePath}/api/send`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({ message })
        });

        if (response.ok) {
            const data = await response.json();
            document.getElementById('message').innerText = `Server Response: ${data.response}`;
        } else {
            document.getElementById('message').innerText = 'Failed to send message to the server.';
        }
    } catch (error) {
        document.getElementById('message').innerText = 'Error sending message to the server.';
        console.error('Error sending message:', error);
    }
}

function handleButtonClick(i) {
    alert("Button " + i + " clicked!");
}

// Create sample boxes with buttons
for (let i = 1; i <= 5; i++) {
    const div = document.createElement("div");
    div.className = "box";
    div.textContent = "Box " + i + " ";

    const btn = document.createElement("button");
    btn.textContent = "Click Me";
    btn.onclick = () => handleButtonClick(i);

    div.appendChild(btn);
    container.appendChild(div);
}

async function sendMessage() {
    const message = document.getElementById('userMessage').value;

    if (message) {
        try {
            const response = await fetch(`${basePath}/api/send`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({ message })
            });

            if (response.ok) {
                const data = await response.json();
                document.getElementById('message').innerText = `Server Response: ${data.response}`;
            } else {
                document.getElementById('message').innerText = 'Failed to send message to the server.';
            }
        } catch (error) {
            document.getElementById('message').innerText = 'Error sending message to the server.';
            console.error('Error sending message:', error);
        }
    } else {
        document.getElementById('message').innerText = 'Please enter a message to send.';
    }
}