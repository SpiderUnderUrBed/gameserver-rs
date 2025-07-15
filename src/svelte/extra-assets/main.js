class ServerConsole {
  constructor(basePath = "") {
    this.basePath = basePath;
    this.consoleInput = document.querySelector(".console-input");
    this.historyContainer = document.querySelector(".console-history");
    this.toggablePages = document.getElementById("toggablePages");

    this.globalWs = null;
    this.rawOutputEnabled = false;
    // this.reconnectAttempts = 0;

    this.messageQueue = [];
    this.isProcessingQueue = false;
    this.MAX_RETRIES = 3;
    this.RETRY_DELAY = 100;

    this.initialize();
  }

  initialize() {
    this.setupInputListener();
    this.connectWebSocket();

    
    window.toggleNodes = () => this.toggleNodes();
    window.toggleRaw = () => this.toggleRaw();
    window.addMore = () => this.addMore();
    window.startServer = () => this.startServer();
    window.createDefaultServer = () => this.createDefaultServer();
    window.enableDeveloperOptions = () => this.enableDeveloperOptions();
  }

  setupInputListener() {
    if (this.consoleInput && this.historyContainer) {
      this.consoleInput.addEventListener("keyup", (e) => {
        const code = this.consoleInput.value.trim();
        if (code.length === 0) return;

        if (e.key === "Enter") {
          if (this.globalWs?.readyState === WebSocket.OPEN) {
            this.globalWs.send(
              JSON.stringify({
                type: "console",
                message: code,
                authcode: "0",
              })
            );
          } else {
            console.error("WebSocket not connected");
            this.addResult("", "Error: Not connected to server", false, true);
          }

          this.addResult(code, "", true, false);
          this.consoleInput.value = "";
        }
      });
    }
  }

  connectWebSocket() {
    if (this.globalWs) this.globalWs.close();

    this.globalWs = new WebSocket(`${this.basePath}/api/ws`);

    this.globalWs.addEventListener("open", () => {
      console.log("WebSocket connected");
      this.addResult("", "Connected to server", false, true);
    //   this.reconnectAttempts = 0;
    });

    this.globalWs.addEventListener("message", (e) => {
      const lines = e.data.split("\n");
      lines.forEach((line) => {
        if (line.trim() === "") return;

        if (this.rawOutputEnabled) {
          this.addResult("", line, false, true);
          return;
        }

        try {
          const parsed = JSON.parse(line);
          this.processMessage(parsed);
        } catch {
          const cleaned = this.cleanOutput(line);
          if (cleaned) {
            this.addResult("", cleaned, false, true);
          }
        }
      });
    });

    this.globalWs.addEventListener("close", (event) => {
      console.log("WebSocket disconnected", event.code, event.reason);
      this.addResult("", "Disconnected from server", false, true);
      this.connectWebSocket()
    //   this.reconnectAttempts++;
    //   const retryIn = Math.min(30000, 1000 * 2 ** this.reconnectAttempts);
    //   setTimeout(() => this.connectWebSocket(), retryIn);
    });

    this.globalWs.addEventListener("error", (err) => {
      console.error("WebSocket error:", err);
      this.addResult("", `WebSocket error: ${err.message}`, false, true);
    });
  }

  cleanOutput(str) {
    return str
      .replace(/\\t/g, "\t")
      .replace(/\\\\/g, "\\")
      .replace(/^\[Server\] ?/, "")
      .trim();
  }

  processMessage(parsed) {
    let output = parsed;

    while (output && typeof output === "object" && "data" in output) {
      if (typeof output.data === "string") {
        try {
          output = JSON.parse(output.data);
        } catch {
          output = output.data;
          break;
        }
      } else {
        output = output.data;
      }
    }

    if (typeof output === "object") {
      output = output.message || output.response || JSON.stringify(output);
    }

    const cleaned = this.cleanOutput(output.toString());
    if (cleaned) this.addResult("", cleaned, false, true);
  }

  async addResult(input, output, addInput, addOutput, retryCount = 0) {
    try {
      const outputAsString =
        output === undefined
          ? "undefined"
          : output === null
          ? "null"
          : Array.isArray(output)
          ? `[${output.join(",")}]`
          : output.toString();

      const isAtBottom =
        this.historyContainer.scrollHeight - this.historyContainer.scrollTop <=
        this.historyContainer.clientHeight + 5;

      if (addInput) {
        const inputEl = document.createElement("div");
        inputEl.className = "console-input-log";
        inputEl.textContent = `> ${input}`;
        this.historyContainer.append(inputEl);
      }

      if (addOutput) {
        const outputEl = document.createElement("div");
        outputEl.className = "console-output-log";
        outputEl.textContent = outputAsString;
        this.historyContainer.append(outputEl);
      }

      if (isAtBottom) {
        this.historyContainer.scrollTop = this.historyContainer.scrollHeight;
      }
    } catch (error) {
      console.error("addResult error:", error);
      if (retryCount < this.MAX_RETRIES) {
        this.messageQueue.push({
          input,
          output,
          addInput,
          addOutput,
          retryCount: retryCount + 1,
        });
        if (!this.isProcessingQueue) this.processQueue();
      }
    }
  }

  async processQueue() {
    if (this.isProcessingQueue || this.messageQueue.length === 0) return;

    this.isProcessingQueue = true;
    try {
      while (this.messageQueue.length > 0) {
        const msg = this.messageQueue.shift();
        await this.addResult(
          msg.input,
          msg.output,
          msg.addInput,
          msg.addOutput,
          msg.retryCount
        );
        await new Promise((res) => setTimeout(res, this.RETRY_DELAY));
      }
    } finally {
      this.isProcessingQueue = false;
    }
  }

  toggleRaw() {
    this.rawOutputEnabled = !this.rawOutputEnabled;
    const rawButton = document.querySelector(".raw-toggle-button");
    if (rawButton) {
      rawButton.textContent = this.rawOutputEnabled
        ? "Raw Output: ON"
        : "Raw Output: OFF";
    }
    this.addResult(
      "",
      `Raw output ${this.rawOutputEnabled ? "enabled" : "disabled"}`,
      false,
      true
    );
  }

  toggleNodes() {
    const nodesBar = document.querySelector("#nodes-bar");
    if (nodesBar) {
        if (nodesBar.classList.contains('visible')) {
            nodesBar.classList.remove('visible');
        } else {
            nodesBar.classList.add('visible');
        }
    }
  }

  addMore() {
    console.log("Add more functionality");
  }

  enableDeveloperOptions() {
    if (!this.toggablePages) return;
    this.toggablePages.style.display =
      this.toggablePages.style.display === "flex" ? "none" : "flex";
  }

  async startServer() {
    try {
      const res = await fetch(`${this.basePath}/api/general`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          kind: "IncomingMessage",
          data: { type: "command", message: "start_server", authcode: "0" },
        }),
      });

      const text = await res.text();
      if (res.ok) {
        try {
          const data = JSON.parse(text);
          this.addResult("", `Server Response: ${data.response}`, false, true);
        } catch {
          this.addResult("", `Invalid JSON response: ${text}`, false, true);
        }
      } else {
        this.addResult("", `Failed (${res.status}): ${text}`, false, true);
      }
    } catch (err) {
      this.addResult("", `Error: ${err.message}`, false, true);
    }
  }

  async createDefaultServer() {
    try {
      const res = await fetch(`${this.basePath}/api/general`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          kind: "IncomingMessage",
          data: {
            message: "create_server",
            type: "command",
            authcode: "0",
          },
        }),
      });

      if (res.ok) {
        try {
          const data = await res.json();
          this.addResult("", `Server Response: ${data.response}`, false, true);
        } catch {
          const text = await res.text();
          this.addResult("", `Success, but invalid JSON: ${text}`, false, true);
        }
      } else {
        const text = await res.text();
        this.addResult("", `Failed (${res.status}): ${text}`, false, true);
      }
    } catch (err) {
      this.addResult("", `Error: ${err.message}`, false, true);
    }
  }
}


document.addEventListener("DOMContentLoaded", () => {
  new ServerConsole();
});
