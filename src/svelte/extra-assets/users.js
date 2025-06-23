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
                    clone.getElementById('roles').textContent = user.user_perms;
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

async function extraSettings(button){
    const username = button.closest("[data-user-container]").querySelector("#username").innerText;
    const currentuserperms = document.getElementById('current-user-perms');
    document.getElementById('user').innerText = username;
    document.getElementById('globalUserDialog').showModal()
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
            if (json.user_perms.length == 0) {
                currentuserperms.innerText = "No Roles at the moment"
            } else {
                currentuserperms.innerText = json.user_perms.join(",");
            }
        } else {
            console.error(response.body);
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
async function addPerms(){
    let perms_div = document.getElementById("perms");
    let permission = perms_div.value;
    let permission_text = perms_div.options[perms_div.selectedIndex].text;
    console.log(permission);

    let perm_div = document.getElementById("current-perms");
    perm_div.innerHTML += `<div class="perm-item" data-value=${permission}>${permission_text}</div>`;
}
async function addUser(){
    event.preventDefault()
    console.log("adding user");
    
    const user = document.getElementById('create-username').value;
    const password = document.getElementById('password').value;
    const jwt = "";
    const user_perms = [];
    const user_perms_div = document.getElementById("current-perms");
    for (let child of user_perms_div.children){
        console.log(`Permission: ${child.dataset.value}`);
        user_perms.push(child.dataset.value);
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
                data: { user, password, user_perms }
            },
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
    
    try {
        const response = await fetch(`${basePath}/api/edituser`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({
                element: { user, password, user_perms },
                jwt
            })
        });

        if (!response.ok) {
            const error = await response.text();
            console.error('Server error:', error);
            alert('Failed to delete user.');
        } else {
            const result = await response.text();
            console.log('User deleted:', result);
            alert('User delete successfully!');
            fetchUsers()
        }
    } catch (err) {
        console.error('Request failed:', err);
        alert('An error occurred while creating the user.');
    }
}
async function deleteUser(user){
    const jwt = "";
    try {
        const response = await fetch(`${basePath}/api/deleteuser`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({ element: user, jwt })
        });

        if (!response.ok) {
            const error = await response.text();
            console.error('Server error:', error);
            alert('Failed to delete user.');
        } else {
            const result = await response.text();
            console.log('User deleted:', result);
            alert('User delete successfully!');
            fetchUsers()
        }
    } catch (err) {
        console.error('Request failed:', err);
        alert('An error occurred while creating the user.');
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