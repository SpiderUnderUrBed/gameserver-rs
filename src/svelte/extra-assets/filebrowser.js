const basePath = document.querySelector('meta[name="site-url"]').content.replace(/\/$/, '');
let previous_path = "";
let globalWs = null;

function connectWebSocket() {
  globalWs = new WebSocket(`${basePath}/api/ws`);

  globalWs.addEventListener("open", () => {
    console.log("WebSocket connected");
  });

  globalWs.addEventListener("close", (event) => {
    console.log("WebSocket disconnected", event.code, event.reason);
    setTimeout(connectWebSocket, 2000);
  });

  globalWs.addEventListener("error", (err) => {
    console.error("WebSocket error:", err);
  });
}

connectWebSocket();

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
        filelist.unshift({ kind: "Folder", data: ".." });

        for (let i = 0; i < filelist.length; i++) {
          console.log(filelist[i])
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
              console.log(filename);
            };
          }

          newelement.appendChild(button);
          button.textContent = filename;
          fileview.appendChild(newelement);
        }
      } catch {
        console.log(`Invalid JSON response: ${text}`);
      }
    } else {
      console.warn(`Failed (${res.status}): ${text}`);
      // Only retry if it's NOT a hard failure like 403
      if (res.status !== 403) {
        setTimeout(() => {
          if (previous_path !== path) {
            get_files(previous_path);
          }
        }, 2000);
      } else {
        console.error("403 Forbidden — stopping retries.");
      }
    }
  } catch (err) {
    console.log(`Error: ${err.message}`);
    // Retry after delay if not same path to avoid infinite loop
    setTimeout(() => {
      if (previous_path !== path) {
        get_files(previous_path);
      }
    }, 2000);
  }
}

get_files("");
