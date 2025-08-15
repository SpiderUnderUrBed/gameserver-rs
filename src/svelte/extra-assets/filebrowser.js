Dropzone.autoDiscover = false;

const basePath = document.querySelector('meta[name="site-url"]').content.replace(/\/$/, '');
let previous_path = "";
let globalWs = null;

function connectWebSocket() {
  globalWs = new WebSocket(`${basePath}/api/ws`);

  globalWs.addEventListener("open", () => console.log("WebSocket connected"));
  globalWs.addEventListener("close", (event) => {
    console.log("WebSocket disconnected", event.code, event.reason);
    setTimeout(connectWebSocket, 2000);
  });
  globalWs.addEventListener("error", (err) => console.error("WebSocket error:", err));
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
      body: JSON.stringify({ type: "command", message: path, authcode: "0" }),
    });

    const text = await res.text();
    if (res.ok) {
      try {
        const data = JSON.parse(text);
        let filelist = data.list.data;
        filelist.unshift({ kind: "Folder", data: ".." });

        filelist.forEach(item => {
          let filename = item.data;
          let newelement = document.createElement('div');
          let button = document.createElement('button');

          if (item.kind.toLowerCase() == "folder") {
            button.onclick = () => {
              previous_path = path;
              get_files(path + '/' + filename);
            };
          } else {
            button.onclick = () => console.log(filename);
          }

          newelement.appendChild(button);
          button.textContent = filename;
          fileview.appendChild(newelement);
        });
      } catch {
        console.log(`Invalid JSON response: ${text}`);
      }
    } else {
      console.warn(`Failed (${res.status}): ${text}`);
    }
  } catch (err) {
    console.log(`Error: ${err.message}`);
  }
}

get_files("");

const dz = new Dropzone("#myDropzone", {
  url: `${basePath}/api/upload`,
  paramName: "file",
  method: "post",
  maxFilesize: 1024, 
  parallelUploads: 10,
  uploadMultiple: false,
  addRemoveLinks: true,
  dictDefaultMessage: "Drop files or folders here to upload",
  init: function() {
    this.on("success", function(file, response) {
      console.log("Uploaded:", file.name, response);
      get_files(previous_path); 
    });
    this.on("error", function(file, err) {
      console.error("Upload error:", err);
    });
  }
});

dz.hiddenFileInput.setAttribute("webkitdirectory", true);
