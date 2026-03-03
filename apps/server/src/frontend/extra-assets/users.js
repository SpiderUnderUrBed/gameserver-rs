const user_div = document.querySelector(".users");
const basePath = document.querySelector('meta[name="site-url"]').content.replace(/\/$/, '');
    

async function fetchUsers() {
    try {
        const response = await fetch(`${basePath}/api/users`);
        if (response.ok) {
            user_div.innerHTML = "";

            const data = await response;
            const json_data = await  data.json();
            const users = json_data.list.data;

            users.forEach((user, index) => {
                const userbutton = document.createElement("div");
                userbutton.role = "button";
            
                const clone = document.getElementById("user-element-inner-template").content.cloneNode(true);
                clone.getElementById('username').textContent = user.username;
                if (user.user_perms.length != 0) {
                    clone.getElementById('roles').textContent = user.user_perms.join(", ");
                }
                userbutton.appendChild(clone);
                // userbutton.className = "users-element";
                user_div.appendChild(userbutton);
            });
        } else {
            console.log("Failed to get users from the server");
            document.getElementById('message').innerText = 'Failed to get users from the server.';
        }
    } catch (error) {
        document.getElementById('message').innerText = 'Error connecting to the server.';
        console.error('Error fetching users:', error);
    }
}
fetchUsers()

async function extraSettings(button) {
    const username = button.closest("[data-user-container]").querySelector("#username").innerText;
    const currentuserperms = document.getElementById('perms');
    document.getElementById('user').innerText = username;
    document.getElementById('globalUserDialog').showModal();
    
    try {
        const response = await fetch(`${basePath}/api/getuser`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({ user: username })
        });
        
        if (response.ok) {
            const json = await response.json();
            
            while (currentuserperms.children.length > 2) {
                currentuserperms.removeChild(currentuserperms.lastChild);
            }
            
            const noRoleButton = document.getElementById("no-roles");
            if (noRoleButton) {
                noRoleButton.remove();
            }
            
            if (json.user_perms.length === 0) {
                const internal_content = document.createElement("div");
                internal_content.textContent = "No Roles at the moment";
                internal_content.id = "no-roles";
                currentuserperms.appendChild(internal_content);
            } else {
                const uniquePerms = [...new Set(json.user_perms)];
                
                uniquePerms.forEach(perm => {
                    const btn = document.createElement("button");
                    btn.className = "perm-item";
                    btn.dataset.value = perm;
                    btn.textContent = perm;
                    btn.onclick = function() {
                        deletePerm(btn);
                    };
                    currentuserperms.appendChild(btn);
                });
            }
        } else {
            console.error(response.body);
            currentuserperms.innerText = "Error getting user data";
        }
    } catch (error) {
        currentuserperms.innerText = "Error getting data from the server";
        console.error(error);
    }
}

async function togglePassword(button){
    const img = button.querySelector("img");
    if (img.src.includes("show.svg")) {
        try {
            const username = button.closest("[data-user-container]").querySelector("#username").innerText;
            const response = await fetch(`${basePath}/api/getuser`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({ user: username })
                });
            if (response.ok) {
                const json = await response.json();
                console.log(json);
            } else {
                console.error(response.body);
            }
        } catch (e) {
            console.error(`Error: ${e}`);
        }
        img.src = "icons/hide.svg"
    } else {
        img.src = "icons/show.svg"
    }
}
async function addPerms(button){
    let root = button.closest("[data-user-container]");
    const perms_div = root.querySelector("#perms-selector");

    const permission = perms_div.value;
    const permission_text = perms_div.options[perms_div.selectedIndex].text;

    const display_perms_div = root.querySelector("#perms");

    const btn = document.createElement("button");
    btn.className = "perm-item";
    btn.dataset.value = permission;
    btn.textContent = permission_text;
    btn.onclick = function () {
        deletePerm(btn);
    };

    display_perms_div.appendChild(btn);
    //}
}

async function deletePerm(button){
    console.log("Deleting perm");
    button.remove();
}
async function addUser(){
    event.preventDefault()
    console.log("adding user");
    
    const user = document.getElementById('create-username').value;
    const password = document.getElementById('password').value;
    const jwt = "";
    const user_perms = [];
    const user_perms_div = document.getElementById("perms");
    for (let child of user_perms_div.children){
        console.log(`Permission: ${child.dataset.value}`);
        if (child.dataset.value != undefined) {
            user_perms.push(child.dataset.value);
        }
    }

    try {
        const response = await fetch(`${basePath}/api/createuser`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json'
        },
        body: JSON.stringify({
            element: { 
                kind: "User", 
                data: { 
                    user, password, user_perms
                }
            },
            // To be clear, just because its set to true at this point in the code, does not mean it gets to 
            // demand the server to not require auth to prevent spoofing, the only time it respects that request is if its
            // made internally
            require_auth: true,
            jwt
        })
        });

        if (!response.ok) {
        const error = await response.text();
        console.error('Server error:', error);
        alert('Failed to create user.');
        } else {
        const result = await response.text();
        console.log('User created:', result);
        // fetchUsers()
        alert('User created successfully!');
        fetchUsers()
        }
    } catch (err) {
        console.error('Request failed:', err);
        alert('An error occurred while creating the user.');
    }
}
async function editPermissions(button){
    const user = button.closest("[data-user-container]").querySelector("#user").innerText;
    
    let jwt = "";
    let user_perms = [];
    let password = "";

    for (let child of button.closest("[data-user-container]").querySelector("#perms").children){
        if (child.dataset.value != undefined) {
            user_perms.push(child.dataset.value);
        }
    }
    
    try {
        const response = await fetch(`${basePath}/api/edituser`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({
                element: { 
                    data: {
                        user, password, user_perms
                    }, 
                    kind: "User" 
                },
                // To be clear, just because its set to true at this point in the code, does not mean it gets to 
                // demand the server to not require auth to prevent spoofing, the only time it respects that request is if its
                // made internally
                require_auth: true,
                jwt
            })
        });

        if (!response.ok) {
            const error = await response.text();
            console.error('Server error:', error);
            alert('Failed to edit user.');
        } else {
            const result = await response.text();
            console.log('User edited:', result);
            alert('User edited successfully!');
            fetchUsers()
        }
    } catch (err) {
        console.error('Request failed:', err);
        alert('An error occurred while editing the user.');
    }
}

async function deleteUser(username) {
    const jwt = "";

    try {
        const response = await fetch(`${basePath}/api/deleteuser`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({
                element: {
                    kind: "User",
                    data: {
                        user: username,
                        password: "",   
                        user_perms: []  
                    }
                },
                jwt,
                require_auth: false
            })
        });

        if (!response.ok) {
            const error = await response.text();
            console.error('Server error:', error);
            alert('Failed to delete user.');
        } else {
            const result = await response.text();
            console.log('User deleted:', result);
            alert('User deleted successfully!');
            fetchUsers();
        }
    } catch (err) {
        console.error('Request failed:', err);
        alert('An error occurred while deleting the user.');
    }
}

async function deleteUserByDialog(){
    event.preventDefault()
    
    const user = document.getElementById('delete-username').value;
    // const user_perms = [];
    deleteUser(user);
}
async function deleteUserByClick(button){
    const user = button.closest("[data-user-container]").querySelector("#user").innerText;
    deleteUser(user);
}