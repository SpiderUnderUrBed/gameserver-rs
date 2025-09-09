class ServerConsole {
  constructor(basePath = "") {
    this.basePath = basePath;
    this.consoleInput = document.querySelector(".console-input");
    this.historyContainer = document.querySelector(".console-history");
    this.toggablePages = document.getElementById("toggablePages");

    // this.start_keyword = ""
    // this.stop_keyword = ""

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
    this.fetchNodes();
    this.selectedNodeType();
    this.loadTopmostButtonsLinks();
    this.setStatuses();
    this.loadFileUpload();

   window.nodeClicked = (button) => this.nodeClicked(button);
    window.updateStatus = () => this.updateStatus();  
    window.deleteServer = () => this.deleteServer();
    window.updateServer = () => this.updateServer();
    window.configureServer = () => this.configureServer();
    window.restoreButtonDefaults = () => this.restoreButtonDefaults();
    window.temporaryButtonReset = () => this.temporaryButtonReset();
    window.toggleButtons = () => this.toggleButtons();
    window.configuredTopmostButtons = () => this.configuredTopmostButtons();
    window.configureTopmostButtons = () => this.configureTopmostButtons();
    window.stopServer = () => this.stopServer();
    window.addNode = () => this.addNode();
    window.toggleNodes = () => this.toggleNodes();
    window.toggleRaw = () => this.toggleRaw();
    window.addMore = () => this.addMore();
    window.startServer = () => this.startServer();
    window.createDefaultServer = () => this.createDefaultServer();
    window.enableDeveloperOptions = () => this.enableDeveloperOptions();
  }
  
async loadFileUpload() {
  // if (typeof Dropzone === "undefined") {
  //   console.error("Dropzone not loaded");
  //   return;
  // }

  // if (Dropzone.instances.length) {
  //   Dropzone.instances.forEach(dz => dz.destroy());
  // }

  const dz = new Dropzone("#myDropzone", {
    url: `${this.basePath}/api/upload`,
    paramName: "file",
    method: "post",
    autoProcessQueue: false,  
    maxFilesize: 1024,         
    parallelUploads: 10,
    uploadMultiple: false,
    addRemoveLinks: true,
    dictDefaultMessage: "Drop files or folders here to upload",
    init: function () {
      this.on("success", (file, response) => {
        console.log("Upload success:", file.name, response);
      });
      this.on("error", (file, errorMessage) => {
        console.error("Upload error:", file.name, errorMessage);
      });
      this.on("queuecomplete", () => {
        console.log("All uploads complete");
      });
    }
  });

  dz.hiddenFileInput.setAttribute("webkitdirectory", true);

  document.getElementById("uploadBtn").addEventListener("click", () => {
    if (dz.getQueuedFiles().length === 0) {
      alert("No files to upload.");
      return;
    }
    dz.processQueue();
  });
  }


  async setStatuses(){
    let button_status = document.getElementById("temp-enable-defaults");
      try {
        const res = await fetch(`${this.basePath}/api/getstatus`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            type: "buttons", message: "", authcode: "0",
          }),
        });

      const text = await res.text();
      if (res.ok) {
        try {
          const data = JSON.parse(text);
          console.log(`Server Response: ${data.response}`)
          if (data.message == "true") {
            button_status.style.backgroundColor = "green";
          } else {
            button_status.style.backgroundColor = "red";
          }
        } catch {
          console.log(`Invalid JSON response: ${text}`)
        }
      } else {
        console.log(`Failed (${res.status}): ${text}`)
      }
    } catch (err) {
      console.log(`Error: ${err.message}`)
    }   
    // let server_status = document.getElementById("server-status-indicator");
    this.updateStatus("none", true);
  }
  selectedNodeType(){
    const selector = document.getElementById("nodetype-selector");
    const ip = document.getElementById("nodeip");
    
    selector.addEventListener("change", function () {
        const selectedValue = selector.value;

        if (selectedValue === "custom") {
            console.log("Custom selected");
            ip.disabled = false;
        } else if (selectedValue === "main") {
            console.log("Main selected");
            ip.value = "";
            ip.disabled = true;
        }
    });
  }


async changeNode(node) {
    try {
      let button_status = document.getElementById("temp-enable-defaults");
      console.log("Changing node")
      const res = await fetch(`${this.basePath}/api/changenode`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          type: "changenode", message: node, authcode: "0",
        }),
      });
      console.log("After req")

      const text = await res.text();
      if (res.ok) {
        try {
          //const data = JSON.parse(text);
          console.log("Response is ok")
          this.addResult(`Changed to node: ${node}`)
          this.updateStatus("up", false)
        } catch {
          console.log(`Invalid JSON response: ${text}`)
        }
      } else {
        console.log(`Failed (${res.status}): ${text}`)
      }
    } catch (err) {
      console.log(`Error: ${err.message}`)
    }   
}

  
async nodeClicked(button) {
  console.log("Button element:", button);

  const nodeEl = button.querySelector("#node-dialog-name");
  if (!nodeEl) {
    console.error("No #node element inside button", button);
    return;
  }

  const node = nodeEl.innerText;
  console.log("Clicked node:", node);
}


async fetchNodes() {
    try {
        const response = await fetch(`${this.basePath}/api/nodes`);
        if (!response.ok) throw new Error("Failed to fetch nodes");

        const data = await response.json();
        const nodes = data.list.data;

        const nodesBar = document.querySelector("#nodes-bar");
        let nodeDialog = document.getElementById("nodeActionDialog");

        if (nodesBar) nodesBar.innerHTML = "";
nodes.forEach((node) => {
  if (nodesBar) {
    const button = document.createElement("button");
    button.className = "nodes-element";

    const clone = document
      .getElementById("node-element-inner-template")
      .content.cloneNode(true);

    clone.getElementById("node-dialog-name").textContent = node;
    button.appendChild(clone);

    // button.addEventListener("click", (e) => {
    //   this.nodeClicked(e.currentTarget);
    //   nodeDialog.showModal();
    // });
    
    button.addEventListener("click", () => {
      //this.nodeClicked(e.currentTarget);
      // nodeDialog.showModal();
      nodeDialog.show();
    });

    nodesBar.appendChild(button);
  }
});

        const migrateto = document.getElementById("migrate-to");
        const migratefrom = document.getElementById("migrate-from");

        if (!migrateto || !migratefrom) {
            console.warn("Migration selects not found in DOM yet. Will retry when dialog opens.");
            return;
        }

        migrateto.innerHTML = "";
        migratefrom.innerHTML = "";

        nodes.forEach((node) => {
            const option = document.createElement("option");
            option.value = node;
            option.text = node;

            migrateto.appendChild(option);
            migratefrom.appendChild(option.cloneNode(true));
        });

    } catch (error) {
        console.error("Error fetching nodes:", error);
    }
}



//   setupNodeBar(){
//     const nodesBar = document.querySelector("#nodes-bar");

//   }

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
  async isAllowedInConsoleResult(str){
    console.log(str)
    let isNotAllowed = false
    let rawOutputEnabled = this.rawOutputEnabled;
    console.log("Checking")
    try {
      let json = JSON.parse(str);
      console.log("In try-catch")
      if (!rawOutputEnabled) {
        console.log("Is not allowed")
        isNotAllowed = true;
        // if (json.start_keyword && json.stop_keyword) {
        //   isNotAllowed = true;
        // }  
      }
    } catch (e) {
      //isNotAllowed = false;
      console.log(`${e}`)
    }
    console.log("Returning")
    return isNotAllowed
  }

  async addResult(input, output, addInput, addOutput, retryCount = 0) {
    try {
      if (await this.isAllowedInConsoleResult(output)) {
          return;
          //throw new Error("Input not allowed in console"); 
      }
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

  toggleButtons(){
    const managementdiv = document.getElementById("management-buttons");
    if (managementdiv.style.display == "none") {
      managementdiv.style.display = "block";
    } else {
      managementdiv.style.display = "none";
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
        if (nodesBar.style.display == "none"){
            nodesBar.style.display = "flex";
        } else {
            nodesBar.style.display = "none";
        }
        // if (nodesBar.classList.contains('visible')) {
        //     nodesBar.classList.remove('visible');
        // } else {
        //     nodesBar.classList.add('visible');
        // }
    }
  }
  async temporaryButtonReset(){
    let button_status = document.getElementById("temp-enable-defaults");
    try {
      const res = await fetch(`${this.basePath}/api/buttonreset`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          type: "command", message: "toggle", authcode: "0",
        }),
      });

      const text = await res.text();
      if (res.ok) {
        await this.loadTopmostButtonsLinks();
        if (button_status.style.backgroundColor == "green") {
          button_status.style.backgroundColor = "red"
        } else {
          button_status.style.backgroundColor = "green"
        }
        try {
          const data = JSON.parse(text);
          console.log(`Server Response: ${data.response}`)
        } catch {
          console.log(`Invalid JSON response: ${text}`)
        }
      } else {
        console.log(`Failed (${res.status}): ${text}`)
      }
    } catch (err) {
      console.log(`Error: ${err.message}`)
    }   
  }
  async restoreButtonDefaults(){
    let button_status = document.getElementById("temp-enable-defaults");
    try {
      const res = await fetch(`${this.basePath}/api/buttonreset`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({type: "command", message: "restore", authcode: "0"}),
      });

      
      const text = await res.text();
      if (res.ok) {
        button_status.style.backgroundColor = "red"
        await this.loadTopmostButtonsLinks();
        try {
          const data = JSON.parse(text);
          console.log(`Server Response: ${data.response}`)
        } catch {
          console.log(`Invalid JSON response: ${text}`)
        }
      } else {
        console.log(`Failed (${res.status}): ${text}`)
      }
    } catch (err) {
      console.log(`Error: ${err.message}`)
    }
  }

  // async fetchButtons(){
  //   try {
  //       const response = await fetch(`${this.basePath}/api/buttons`);
  //       if (response.ok) {
  //           const data = await response.json();
  //           const nodes = data.list.data;

  //           // const nodesBar = document.querySelector("#nodes-bar");
  //           // nodesBar.innerHTML = ""; 

  //           // nodes.forEach((node, index) => {
  //           //     const button = document.createElement("button");
  //           //     button.textContent = node;
  //           //     button.className = "nodes-element";
  //           //     button.onclick = () => alert(`Node clicked: ${node}`);
  //           //     nodesBar.appendChild(button);
  //           // });
  //       } else {
  //           // document.getElementById('message').innerText = 'Failed to get nodes from the server.';
  //       }
  //   } catch (error) {
  //       // document.getElementById('message').innerText = 'Error connecting to the server.';
  //       console.log('Error fetching nodes:', error);
  //   }
  // }
  async addNode(){
      event.preventDefault()
      console.log("adding node");
      
      const nodename = document.getElementById('create-nodename').value;
      const nodeip = document.getElementById('nodeip').value;
      const nodetype = document.getElementById('nodetype-selector').value;
      const jwt = "";

      try {
        const response = await fetch(`${this.basePath}/api/addnode`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({
                element: {
                    kind: "Node",
                    data: {
                        nodename, 
                        ip: nodeip, 
                        nodetype: { kind: nodetype, data: null }, 
                        nodestatus: { kind: "Enabled", data: null }
                    }
                },
                jwt,
                require_auth: true
            })
        });
          // To be clear, just because its set to true at this point in the code, does not mean it gets to 
          // demand the server to not require auth to prevent spoofing, the only time it respects that request is if its
          // made internally

          if (!response.ok) {
          const error = await response.text();
          console.error('Server error:', error);
          alert('Failed to create node.');
          } else {
          const result = await response.text();
          console.log('Node created:', result);
          // fetchUsers()
          alert('Node created successfully!');
          this.fetchNodes()
          }
      } catch (err) {
          console.error('Request failed:', err);
          alert('An error occurred while creating the node.');
      }
  };

  async deleteServer(){
    try {
      const res = await fetch(`${this.basePath}/api/general`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          kind: "IncomingMessage",
          data: { 
            type: "command", 
            message: "delete_server", 
            authcode: "0", 
            // metadata: "minecraft"
           },
        }),
      });

      const text = await res.text();
      if (res.ok) {
        try {
          const data = JSON.parse(text);
          console.log( `Server Response: ${data.response}`);
        } catch {
          console.log(`Invalid JSON response: ${text}`);
        }
      } else {
        console.log(`Failed (${res.status}): ${text}`)
      }
    } catch (err) {
      console.log(`Error: ${err.message}`)
    }
  }

   async loadTopmostButtonsLinks() {
    // console.log("Latest");
    
    // const topmostButtonValue = document.getElementById("topmost-button-option").value;
    // const topmostButtonLink = document.getElementById("custom-button-link").value;

    // console.log(`${topmostButtonValue} ${topmostButtonLink}`);

    // const params = new URLSearchParams({
    //   name: topmostButtonValue,
    //   link: topmostButtonLink,
    //   type: "Custom"
    // });

    try {
     //onst res = await fetch(`${this.basePath}/api/buttons?${params.toString()}`, {
      const res = await fetch(`${this.basePath}/api/buttons`, {
        method: "GET"
      });

      const text = await res.text();

      if (res.ok) {
        try {
          const data = JSON.parse(text);
          //console.log(`Server Response: ${JSON.stringify(data)}`);
            for (let i = 0; i < data.list.data.length; i++) {
              const button = data.list.data[i];
              let originalButton = document.getElementById(button.name.toLowerCase());
              if (originalButton) {
                const newbutton = originalButton.cloneNode(true);
                originalButton.replaceWith(newbutton);

                newbutton.addEventListener("click", () => {
                  if (button.type.toLowerCase() == "default") {
                    const buttonLower = button.name.toLowerCase();
                    window.location.href = `${button.name.toLowerCase()}.html`;
                  } else if (button.link) {
                    const isExternal = !button.link.startsWith(window.location.origin);
                    const go = !isExternal || confirm(`You are about to visit an external link:\n\n${button.link}\n\nContinue?`);
                    if (go) {
                      window.location.href = button.link;
                    }
                  }
                });
              }
            }
        } catch {
          console.log(`Success, but invalid JSON: ${text}`)
        }
      } else {
        console.error(`Failed (${res.status}): ${text}`)
      }
    } catch (err) {
      console.error(`Error: ${err.message}`)
    }
  }
  configureTopmostButtons(){
    const topmostDialog = document.getElementById("configureTopmostButtonDialog");
    topmostDialog.showModal()
  }

  async configuredTopmostButtons(){
    const topmostButtonValue = document.getElementById("topmost-button-option").value;
    const topmostButtonLink = document.getElementById("custom-button-link").value;
    try {
      const res = await fetch(`${this.basePath}/api/editbuttons`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          element: { 
            kind: "Button", 
            data: { 
              name: topmostButtonValue,
              link: topmostButtonLink,
              type: "custom"
              // type: {
              //   kind: "Custom",
              //   // data: {
              //   //   message: "create_server",
              //   //   type: "command",
              //   //   authcode: "0",
              //   // },
              // }
            }
          },
          jwt: "",
          require_auth: false
        }),
      });

      if (res.ok) {
          const text = await res.text(); 
          await this.loadTopmostButtonsLinks();
          try {
            const data = JSON.parse(text);
            //console.error(text)
            console.log(`Server Response: ${data.response}`)
          } catch {
            console.error(`Success, but invalid JSON: ${text}`)
          }
      } else {
        const text = await res.text();
        console.error(`Failed (${res.status}): ${text}`)
      }
    } catch (err) {
      console.error(`Error: ${err.message}`)
    }
  }

  addMore() {
    console.log("Add more functionality");
    const addServer = document.getElementById("addNodeDialog");
    addServer.showModal()
  }

  enableDeveloperOptions() {
    if (!this.toggablePages) return;
    this.toggablePages.style.display =
      this.toggablePages.style.display === "flex" ? "none" : "flex";
  }
  updateStatus(state, await) {
    const loading = document.getElementById("loading");
    let server_status = document.getElementById("server-status-indicator");
    if (state != "none" && await) {
      loading.style.display = "block";
    }
    const statusEvent = new EventSource(`${this.basePath}/api/awaitserverstatus`);
    statusEvent.onmessage = (e) => {
      console.log(e.data)
      if ((e.data == "healthy" || e.data == "up") && state == "up"){
        loading.style.display = "none";
      } else if (e.data == "down" && state == "down"){
        loading.style.display = "none";
      }
      if ((e.data == "healthy" || e.data == "up")) {
        server_status.style.backgroundColor = "green";
      } else if (e.data == "down") {
        server_status.style.backgroundColor = "red";
      }
      if (!await) {
        statusEvent.close();
      }
    };
  }

  async stopServer() {
    this.updateStatus("down", true)
    try {
      const res = await fetch(`${this.basePath}/api/general`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          kind: "IncomingMessage",
          data: { 
            type: "command", 
            message: "stop_server", 
            authcode: "0", 
            // metadata: "minecraft"
           },
        }),
      });

      const text = await res.text();
      if (res.ok) {
        try {
          const data = JSON.parse(text);
          console.log( `Server Response: ${data.response}`);
        } catch {
          console.log(`Invalid JSON response: ${text}`);
        }
      } else {
        console.log(`Failed (${res.status}): ${text}`)
      }
    } catch (err) {
      console.log(`Error: ${err.message}`)
    }
  }

  async updateServer(){
    let migratingto = document.getElementById("migrate-to").value;
    let migratingfrom = document.getElementById("migrate-from").value;
    try {
      const res_migrate_to = await fetch(`${this.basePath}/api/fetchnode`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          type: "", message: migratingto, authcode: "0",
        }),
      });
      const res_migrate_from = await fetch(`${this.basePath}/api/fetchnode`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          type: "", message: migratingfrom, authcode: "0",
        }),
      });
      // console.log(res_migrate_from)
      // console.log(res_migrate_to)
      const res_migrate_to_json = await res_migrate_to.json();
      const res_migrate_from_json = await res_migrate_from.json();
        const res = await fetch(`${this.basePath}/api/migrate`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
                src: {
                    kind: "Node",
                    data: { 
                        nodename: res_migrate_from_json.nodename, 
                        nodetype: { kind: "Custom", data: null }, 
                        ip: res_migrate_from_json.ip,
                        nodestatus: { kind: "Enabled", data: null }
                    },
                },
                dest: {
                    kind: "Node",
                    data: { 
                        nodename: res_migrate_to_json.nodename, 
                        nodetype: { kind: "Custom", data: null }, 
                        ip: res_migrate_to_json.ip,
                        nodestatus: { kind: "Enabled", data: null }
                    },
                },
                metadata: ""
            }),
        });

      const text = await res.text();
      if (res.ok) {
        try {
          const data = JSON.parse(text);
          console.log( `Server Response: ${data.response}`);
        } catch {
          console.log(`Invalid JSON response: ${text}`);
        }
      } else {
        console.log(`Failed (${res.status}): ${text}`)
      }
    } catch (err) {
      console.log(`Error: ${err.message}`)
    } 
  }

  configureServer(){
    const serverDialog = document.getElementById("configureServerDialog");
    serverDialog.showModal()
  }
  async startServer() {
    this.updateStatus("up", true)
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
    this.updateStatus("up", true)

    let providertype = document.getElementById("providertype-selector").value;

    try {
      const res = await fetch(`${this.basePath}/api/generalwithmetadata`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          kind: "IncomingMessageWithMetadata",
          data: {
            message: "create_server",
            type: "command",
            authcode: "0",
            //metadata: "minecraft"
            metadata: {
              kind: "Server",
              data: {
                // providertype, location, provider, servername
                servername: providertype,
                providertype,
                provider: "",
                location: ""
              }
            }
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