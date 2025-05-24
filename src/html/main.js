const nodes_div = document.querySelector(".nodes");
const container = document.getElementById("container");
const basePath = document.querySelector('meta[name="site-url"]').content.replace(/\/$/, '');

const consoleInput = document.querySelector(".console-input");
const historyContainer = document.querySelector(".console-history");

async function addResult(inputAsString, output, addInput, addOutput) {
    const outputAsString = 
        output === undefined ? "undefined" :
        output === null ? "null" :
        Array.isArray(output) ? `[${output.join(",")}]` :
        output.toString();

    console.log(inputAsString, outputAsString);

    const isAtBottom = historyContainer.scrollHeight - historyContainer.scrollTop <= historyContainer.clientHeight + 5;

    if (addInput) {
        const inputLogElement = document.createElement("div");
        inputLogElement.classList.add("console-input-log");
        inputLogElement.textContent = `> ${inputAsString}`;
        historyContainer.append(inputLogElement);
    }
    if (addOutput) {
        const outputLogElement = document.createElement("div");
        outputLogElement.classList.add("console-output-log");
        outputLogElement.textContent = outputAsString;
        historyContainer.append(outputLogElement);
    }

    if (isAtBottom) {
        historyContainer.scrollTop = historyContainer.scrollHeight;
    }
}

async function websocket() {
    const ws = new WebSocket(`${basePath}/api/ws`);
    
    ws.addEventListener("open", () => {
        console.log("We are connected");
    });

ws.addEventListener("message", e => {
    const jsonStrings = e.data.split(/(?<=})\s*(?={)/);
    
    jsonStrings.forEach(jsonStr => {
        let outputMessage = jsonStr;
        
        try {
            const parsed = JSON.parse(jsonStr);
            
            if (parsed.data) {
                if (typeof parsed.data === 'string') {
                    try {
                        const inner = JSON.parse(parsed.data);
                        outputMessage = inner.data ?? parsed.data;
                    } catch {
                        outputMessage = parsed.data;
                    }
                } else {
                    outputMessage = parsed.data;
                }
            } else {
                outputMessage = parsed.message ?? JSON.stringify(parsed);
            }
        } catch (err) {
            console.warn("Non-JSON or bad format:", err);
            outputMessage = jsonStr;
        }

        const cleanedOutput = outputMessage.replace(/^\[Server\] ?/, "").replace(/\\t/g, "\t").replace(/\\\\/g, "\\");
        addResult("", cleanedOutput, false, true);
    });
});
}
websocket()

consoleInput.addEventListener("keyup", e => {
    const code = consoleInput.value.trim();
    if (code.length === 0) return;
    
    if (e.key === "Enter") {
        const ws = new WebSocket(`${basePath}/api/ws`);
        
        ws.onopen = () => {
            ws.send(JSON.stringify({
                type: "console",
                message: code,
                authcode: "0"
            }));
            ws.close();
        };

        addResult(code, "", true, false);
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
        } else {
            document.getElementById('message').innerText = 'Failed to get nodes from the server.';
        }
    } catch (error) {
        document.getElementById('message').innerText = 'Error connecting to the server.';
        console.error('Error fetching nodes:', error);
    }
}
fetchNodes()

async function startServer() {
    try {
        console.log('Sending request to start server...');
        const response = await fetch(`${basePath}/api/general`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ message: "start_server", type: "command", authcode: "0", kind: "IncomingMessage" }),
        });

        console.log('Response status:', response.status);

        const text = await response.text(); 

        if (response.ok) {
            try {
                const data = JSON.parse(text);
                console.log('Server response data:', data);
                document.getElementById('message').innerText = `Server Response: ${data.response}`;
            } catch {
                document.getElementById('message').innerText = `Invalid JSON response: ${text}`;
            }
        } else {
            document.getElementById('message').innerText = `Failed (${response.status}): ${text}`;
            console.error('Error response text:', text);
        }

    } catch (error) {
        console.error('Fetch error:', error);
        document.getElementById('message').innerText = `Error: ${error.message}`;
    }
}

async function createDefaultServer() {
    try {
        const response = await fetch(`${basePath}/api/general`, { 
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({ 
                message: "create_server",
                type: "command",
                authcode: "0"
            })
        });

        if (response.ok) {
            const data = await response.json();
            document.getElementById('message').innerText = `Server Response: ${data.response}`;
        } else {
            const error = await response.json();
            document.getElementById('message').innerText = `Failed: ${error}`;
        }
    } catch (error) {
        document.getElementById('message').innerText = `Error: ${error.message}`;
        console.error('Error:', error);
    }
}

async function sendMessage() {
    const messageInput = document.getElementById('userMessage');
    const message = messageInput.value.trim();
    
    if (!message) {
        document.getElementById('message').innerText = 'Please enter a message';
        return;
    }

    try {
        const response = await fetch(`${basePath}/api/send`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                message: message,
                type: "message",
                authcode: "0"
            })
        });

        const result = await response.json();
        if (response.ok) {
            document.getElementById('message').innerText = result.response;
        } else {
            document.getElementById('message').innerText = `Error: ${result}`;
        }
    } catch (error) {
        document.getElementById('message').innerText = `Network error: ${error.message}`;
    }
}

function handleButtonClick(i) {
    alert("Button " + i + " clicked!");
}

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