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
        console.error('Error fetching nodes:', error);
    }
}
fetchNodes()

async function startServer() {
    try {
        //Failed (422): Failed to deserialize the JSON body into the target type: missing field `data` at line 1 column 83
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
// async function goToUsers(){
//     fetch('users.html')
//         .then(response => response.text())
//         .then(html => {
//             document.getElementById('inner').innerHTML = html;
//         });
// }

// async function login(){
//     const username = document.querySelector('.user-login').value;
//     const password = document.querySelector('.user-password').value;

//     // Build urlencoded form data string
//     const formBody = new URLSearchParams();
//     formBody.append('user', username);
//     formBody.append('password', password);

//     try {
//         const res = await fetch(`${basePath}/api/signin`, {
//             method: 'POST',
//             headers: {
//                 'Content-Type': 'application/x-www-form-urlencoded',
//             },
//             body: formBody.toString()
//         });

//         if (!res.ok) {
//             throw new Error(`Login failed with status ${res.status}`);
//         }

//         const data = await res.json();
//         const jwtToken = data.response;

//         console.log("JWT Token:", jwtToken);

//         const nextUrl = encodeURIComponent(`${basePath}/main.html`);
//         const jwkToken = encodeURIComponent(jwtToken);

//         window.location.href = `${basePath}/authenticate?next=${nextUrl}&jwk=${jwkToken}`;

//     } catch (err) {
//         console.error('Login error:', err);
//         alert('Login failed: ' + err.message);
//     }
// }



// testing functions

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
