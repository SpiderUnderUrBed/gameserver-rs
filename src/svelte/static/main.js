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

let globalWs = null;
let rawOutputEnabled = false;
let stillOutputDespiteError = false;

async function websocket() {
    if (globalWs) {
        globalWs.close();
    }
    
    globalWs = new WebSocket(`${basePath}/api/ws`);
    
    globalWs.addEventListener("open", () => {
        console.log("WebSocket connected");
        addResult("", "Connected to server", false, true);
    });
    
    globalWs.addEventListener("message", e => {
        if (rawOutputEnabled) {
            
            const lines = e.data.split('\n');
            
            lines.forEach(line => {
                if (line.trim() === '') return;  
                
                try {
                    
                    const payload = JSON.parse(line.trim());
                    
                    
                    const result = {
                        payload: payload  
                    };
                    addResult("", JSON.stringify(result, null, 2), false, true);
                } catch (err) {
                    
                    console.log("Failed to parse line:", line);
                    const errorResult = {
                        rawLine: line,
                        parseError: err.message
                    };
                    addResult("", JSON.stringify(errorResult, null, 2), false, true);
                }
            });
            return;
        }
        
        
        console.log("Raw message:", e.data);
        
        
        if (!isPotentialJson(e.data)) {
            const cleanedOutput = cleanOutput(e.data);
            if (stillOutputDespiteError) {
                addResult("", cleanedOutput, false, true);
            }
            return;
        }
        
        
        try {
            
            const parsed = JSON.parse(e.data);
            processMessage(parsed);
        } catch (err) {
            
            try {
                
                const lines = e.data.split('\n');
                for (const line of lines) {
                    if (line.trim() === '') continue;
                    
                    try {
                        const parsed = JSON.parse(line);
                        processMessage(parsed);
                    } catch (lineErr) {
                        
                        const cleanedOutput = cleanOutput(line);
                        if (stillOutputDespiteError) {    
                            addResult("", cleanedOutput, false, true);
                        }
                    }
                }
            } catch (splitErr) {
                console.warn("Failed to process message:", splitErr);
                const cleanedOutput = cleanOutput(e.data);
                if (stillOutputDespiteError) {   
                    addResult("", cleanedOutput, false, true);
                }
            }
        }
    });
    
    globalWs.addEventListener("close", () => {
        console.log("WebSocket disconnected");
        
        setTimeout(websocket, 1000);
    });
    
    globalWs.addEventListener("error", (err) => {
        console.error("WebSocket error:", err);
        
    });
}

function isPotentialJson(str) {
    
    return str.trim().startsWith('{') || str.trim().startsWith('[');
}

function cleanOutput(str) {
    return str.replace(/\\t/g, "\t")
              .replace(/\\\\/g, "\\")
              .replace(/^\[Server\] ?/, "");
}

function processMessage(parsed) {
    let outputMessage;
    
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

    const cleanedOutput = cleanOutput(outputMessage);
    addResult("", cleanedOutput, false, true);
}

function toggleRaw() {
    rawOutputEnabled = !rawOutputEnabled;
    const rawButton = document.querySelector('.raw-toggle-button');
    if (rawButton) {
        rawButton.textContent = rawOutputEnabled ? 'Raw Output: ON' : 'Raw Output: OFF';
    }
    addResult("", `Raw output ${rawOutputEnabled ? 'enabled' : 'disabled'}`, false, true);
    return rawOutputEnabled;
}


websocket();
consoleInput.addEventListener("keyup", e => {
    const code = consoleInput.value.trim();
    if (code.length === 0) return;
    
    if (e.key === "Enter") {
        if (globalWs && globalWs.readyState === WebSocket.OPEN) {
            globalWs.send(JSON.stringify({
                type: "console",
                message: code,
                authcode: "0"
            }));
        } else {
            console.error("WebSocket not connected");
            
        }

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
            const nodes = data.list.data;

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
        console.log('Error fetching nodes:', error);
    }
}
fetchNodes()

async function startServer() {
    try {
        
        console.log('Sending request to start server...');
        const response = await fetch(`${basePath}/api/general`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ kind: "IncomingMessage", data: { type: "command", message: "start_server", authcode: "0" }}),
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
                kind: "IncomingMessage", 
                data: {
                    message: "create_server",
                    type: "command",
                    authcode: "0",
                }
            })
        });

        if (response.ok) {
            try {
                const data = await response.json();
                document.getElementById('message').innerText = `Server Response: ${data.response}`;
            } catch (parseError) {
                const text = await response.text();
                document.getElementById('message').innerText = `Success, but invalid JSON: ${text}`;
            }
        } else {
            try {
                const text = await response.text();
                document.getElementById('message').innerText = `Failed (${response.status}): ${text}`;
            } catch (err) {
                document.getElementById('message').innerText = `Unknown error occurred`;
            }
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

document.addEventListener('DOMContentLoaded', function() {
    const container = document.getElementById("container");
    
    for (let i = 1; i <= 5; i++) {
        const div = document.createElement("div");
        div.className = "box";
        div.textContent = "Box " + i + " ";

        const btn = document.createElement("button");
        btn.textContent = "Click Me";
        btn.onclick = () => handleButtonClick(i);

        div.appendChild(btn);
        if (container) {
            container.appendChild(div);
        } else {
            console.log("Container element not found");
        }
    }
});