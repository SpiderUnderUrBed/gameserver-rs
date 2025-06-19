const user_div = document.querySelector(".users");
const basePath = document.querySelector('meta[name="site-url"]').content.replace(/\/$/, '');
    

async function fetchUsers() {
    try {
        const response = await fetch(`${basePath}/api/users`);
        if (response.ok) {
            const data = await response;
            const json_data = await  data.json();
            const users = json_data.list.data;

            user_div.innerHTML = ""; 

            users.forEach((user, index) => {
                const button = document.createElement("div");
                button.role = "button";
                button.innerHTML = `<div style="width: 20px"></div><h5>${user.username}</h5>`;
                button.className = "users-element";
                button.onclick = () => {
                    document.getElementById('user').textContent = user.username;
                    document.getElementById('globalUserDialog').showModal();
                };
                user_div.appendChild(button);
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

async function addPerms(){
    let permission = document.getElementById("perms").value;
    console.log(permission);

    let perm_div = document.getElementById("current-perms");
    perm_div.innerHTML += `<div class="perm-item">${permission}</div>`;
}
async function addUser(){
    event.preventDefault()
    console.log("adding user");
    
    const user = document.getElementById('create-username').value;
    const password = document.getElementById('password').value;
    const authcode = "0";

    console.log(user);

    try {
        const response = await fetch(`${basePath}/api/createuser`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json'
        },
        body: JSON.stringify({ user, password, authcode })
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
async function deleteUser(){
    event.preventDefault()
    
    const user = document.getElementById('delete-username').value;
    const authcode = "0";

    try {
        const response = await fetch(`${basePath}/api/deleteuser`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json'
        },
        body: JSON.stringify({ user, authcode })
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