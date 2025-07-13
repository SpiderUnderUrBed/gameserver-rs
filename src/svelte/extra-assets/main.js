(() => {
    const basePath = "";
    const consoleInput = document.querySelector(".console-input");
    const historyContainer = document.querySelector(".console-history");
    const toggablePages = document.getElementById("toggablePages");

    let globalWs = null;
    let rawOutputEnabled = false;
    let reconnectAttempts = 0;

    const messageQueue = [];
    let isProcessingQueue = false;
    const MAX_RETRIES = 3;
    const RETRY_DELAY = 100; 

    async function addResult(inputAsString, output, addInput, addOutput, retryCount = 0) {
        try {
            const outputAsString = 
                output === undefined ? "undefined" :
                output === null ? "null" :
                Array.isArray(output) ? `[${output.join(",")}]` :
                output.toString();

            console.log(inputAsString, outputAsString);

            if (!historyContainer) {
                if (retryCount < MAX_RETRIES) {
                    messageQueue.push({ inputAsString, output, addInput, addOutput, retryCount: retryCount + 1 });
                    if (!isProcessingQueue) processQueue();
                }
                return;
            }

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
        } catch (error) {
            console.error("Error adding result:", error);
            if (retryCount < MAX_RETRIES) {
                messageQueue.push({ inputAsString, output, addInput, addOutput, retryCount: retryCount + 1 });
                if (!isProcessingQueue) processQueue();
            }
        }
    }

    async function processQueue() {
        if (isProcessingQueue || messageQueue.length === 0) return;
        
        isProcessingQueue = true;
        try {
            while (messageQueue.length > 0) {
                const message = messageQueue.shift();
                await addResult(
                    message.inputAsString,
                    message.output,
                    message.addInput,
                    message.addOutput,
                    message.retryCount
                );
                await new Promise(resolve => setTimeout(resolve, RETRY_DELAY));
            }
        } finally {
            isProcessingQueue = false;
        }
    }

    function enableDeveloperOptions() {
        if (!toggablePages) return;
        toggablePages.style.display = toggablePages.style.display === "flex" ? "none" : "flex";
    }

    function websocket() {
        if (globalWs) {
            globalWs.close();
        }

        globalWs = new WebSocket(`${basePath}/api/ws`);

        globalWs.addEventListener("open", () => {
            console.log("WebSocket connected");
            addResult("", "Connected to server", false, true);
            reconnectAttempts = 0;
        });

        globalWs.addEventListener("message", e => {
            const lines = e.data.split('\n');
            lines.forEach(line => {
                if (line.trim() === '') return;

                if (rawOutputEnabled) {
                    addResult("", line, false, true);
                    return;
                }

                try {
                    const parsed = JSON.parse(line);
                    processMessage(parsed);
                } catch {
                    // If not JSON, treat as raw message
                    const cleaned = cleanOutput(line);
                    if (cleaned) {
                        addResult("", cleaned, false, true);
                    }
                }
            });
        });

        globalWs.addEventListener("close", event => {
            console.log("WebSocket disconnected", event.code, event.reason);
            addResult("", "Disconnected from server", false, true);
            reconnectAttempts++;
            const retryIn = Math.min(30000, 1000 * 2 ** reconnectAttempts);
            console.log(`Reconnecting in ${retryIn}ms...`);
            setTimeout(websocket, retryIn);
        });

        globalWs.addEventListener("error", err => {
            console.error("WebSocket error:", err);
            addResult("", `WebSocket error: ${err.message}`, false, true);
        });
    }

    function cleanOutput(str) {
        // Less aggressive cleaning - just handle escape sequences
        return str.replace(/\\t/g, "\t")
                  .replace(/\\\\/g, "\\")
                  .replace(/^\[Server\] ?/, "")
                  .trim();
    }

    function processMessage(parsed) {
        let outputMessage = parsed;

        // Handle nested data structure
        while (outputMessage && typeof outputMessage === 'object' && 'data' in outputMessage) {
            if (typeof outputMessage.data === 'string') {
                try {
                    outputMessage = JSON.parse(outputMessage.data);
                } catch {
                    outputMessage = outputMessage.data;
                    break;
                }
            } else {
                outputMessage = outputMessage.data;
            }
        }

        if (typeof outputMessage === 'object') {
            outputMessage = outputMessage.message || outputMessage.response || JSON.stringify(outputMessage);
        }

        const cleanedOutput = cleanOutput(outputMessage.toString());
        if (cleanedOutput) {
            addResult("", cleanedOutput, false, true);
        }
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

    function toggleNodes() {
        console.log("Toggle nodes functionality");
    }

    function addMore() {
        console.log("Add more functionality");
    }

    async function startServer() {
        try {
            console.log('Sending request to start server...');
            const response = await fetch(`${basePath}/api/general`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ kind: "IncomingMessage", data: { type: "command", message: "start_server", authcode: "0" }}),
            });

            const text = await response.text();

            if (response.ok) {
                try {
                    const data = JSON.parse(text);
                    addResult("", `Server Response: ${data.response}`, false, true);
                } catch {
                    addResult("", `Invalid JSON response: ${text}`, false, true);
                }
            } else {
                addResult("", `Failed (${response.status}): ${text}`, false, true);
            }
        } catch (error) {
            console.error('Fetch error:', error);
            addResult("", `Error: ${error.message}`, false, true);
        }
    }

    async function createDefaultServer() {
        try {
            const response = await fetch(`${basePath}/api/general`, { 
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
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
                    addResult("", `Server Response: ${data.response}`, false, true);
                } catch (parseError) {
                    const text = await response.text();
                    addResult("", `Success, but invalid JSON: ${text}`, false, true);
                }
            } else {
                const text = await response.text();
                addResult("", `Failed (${response.status}): ${text}`, false, true);
            }
        } catch (error) {
            addResult("", `Error: ${error.message}`, false, true);
            console.error('Error:', error);
        }
    }

    document.addEventListener("DOMContentLoaded", () => {
        websocket();

        if (consoleInput && historyContainer) {
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
                        addResult("", "Error: Not connected to server", false, true);
                    }

                    addResult(code, "", true, false);
                    consoleInput.value = "";
                }
            });
        }
    });
})();