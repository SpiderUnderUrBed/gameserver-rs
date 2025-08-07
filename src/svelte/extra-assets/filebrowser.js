const basePath = document.querySelector('meta[name="site-url"]').content.replace(/\/$/, '');
let previous_path = "";

let globalWs = null;
function connectWebSocket(){
  globalWs = new WebSocket(`${basePath}/api/ws`);

  globalWs.addEventListener("open", () => {
    console.log("WebSocket connected");
  //   reconnectAttempts = 0;
  });

  // globalWs.addEventListener("message", (e) => {
  //   const lines = e.data.split("\n");
  //   lines.forEach((line) => {
  //     if (line.trim() === "") return;

  //     if (rawOutputEnabled) {
  //       addResult("", line, false, true);
  //       return;
  //     }

  //     try {
  //       const parsed = JSON.parse(line);
  //       processMessage(parsed);
  //     } catch {
  //       const cleaned = cleanOutput(line);
  //       if (cleaned) {
  //         addResult("", cleaned, false, true);
  //       }
  //     }
  //   });
  // });

  globalWs.addEventListener("close", (event) => {
    console.log("WebSocket disconnected", event.code, event.reason);
    // addResult("", "Disconnected from server", false, true);
    connectWebSocket()
  //   reconnectAttempts++;
  //   const retryIn = Math.min(30000, 1000 * 2 ** reconnectAttempts);
  //   setTimeout(() => connectWebSocket(), retryIn);
  });

  globalWs.addEventListener("error", (err) => {
    console.error("WebSocket error:", err);
    //addResult("", `WebSocket error: ${err.message}`, false, true);
  });
}
connectWebSocket()

async function get_files(path) {
  let fileview = document.getElementById("center");
  fileview.innerHTML = "";
  console.log(basePath);
  try {
    const res = await fetch(`${basePath}/api/getfiles`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        type: "command", message: path, authcode: "0",
      }),
    });

    const text = await res.text();
    if (res.ok) {
      try {
        const data = JSON.parse(text);
        let filelist = data.list.data;
        filelist.unshift({
            kind: "Folder",
            data: ".."
        })

        for (let i = 0; i < filelist.length; i++) {
          let filename = filelist[i].data;
          let newelement = document.createElement('div');
          let button = document.createElement('button');
          if (filelist[i].kind.toLowerCase() == "folder") {
            button.onclick = () => {
                previous_path = path;
                get_files(path + '/' + filename);
            };
          } else {
            button.onclick = () => {
                console.log(filename)
            }
          }
          newelement.appendChild(button);
          button.textContent = filename;
          fileview.appendChild(newelement);
        }

      } catch {
        console.log(`Invalid JSON response: ${text}`);
      }
    } else {
      get_files(previous_path);
      console.log(`Failed (${res.status}): ${text}`);
    }
  } catch (err) {
    console.log(`Error: ${err.message}`);
  }
}

get_files("");
