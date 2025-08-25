use std::process::Command;
use std::collections::HashMap;
use serde::Serialize;
use serde::Deserialize;
use serde;

// const SERVER_DIR: &str = if cfg!(target_os = "windows") {
//     "C:\\minecraft_server"
// } else {
//     "/opt/minecraft_server"
// };
const SERVER_DIR: &str = "server";

pub trait Provider {
    fn pre_hook(&self) -> Option<Command>;
    fn install(&self) -> Option<Command>;
    fn post_hook(&self) -> Option<Command>;
    fn start(&self) -> Option<Command>;
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderConfig {
    pub pre_hook: Option<String>,
    pub install: Option<String>,
    pub post_hook: Option<String>,
    pub start: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProviderType {
    pub name: String,
    pub config: std::collections::HashMap<String, String>,
}

impl ProviderType {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            config: std::collections::HashMap::new(),
        }
    }

    pub fn with_config(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config.insert(key.into(), value.into());
        self
    }

    pub fn get_config(&self, key: &str) -> Option<&String> {
        self.config.get(key)
    }
}

impl Provider for ProviderType {
    fn pre_hook(&self) -> Option<Command> {
        match self.name.as_str() {
            "minecraft" => {
                let minecraft: Minecraft = self.clone().into();
                minecraft.pre_hook()
            }
            "custom" => {
                let custom: Custom = self.clone().into();
                custom.pre_hook()
            }
            _ => None,
        }
    }

    fn install(&self) -> Option<Command> {
        match self.name.as_str() {
            "minecraft" => {
                let minecraft: Minecraft = self.clone().into();
                minecraft.install()
            }
            "custom" => {
                let custom: Custom = self.clone().into();
                custom.install()
            }
            _ => None,
        }
    }

    fn post_hook(&self) -> Option<Command> {
        match self.name.as_str() {
            "minecraft" => {
                let minecraft: Minecraft = self.clone().into();
                minecraft.post_hook()
            }
            "custom" => {
                let custom: Custom = self.clone().into();
                custom.post_hook()
            }
            _ => None,
        }
    }

    fn start(&self) -> Option<Command> {
        match self.name.as_str() {
            "minecraft" => {
                let minecraft: Minecraft = self.clone().into();
                minecraft.start()
            }
            "custom" => {
                let custom: Custom = self.clone().into();
                custom.start()
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Custom {
    pub pre_hook_cmd: Option<String>,
    pub install_cmd: Option<String>,
    pub post_hook_cmd: Option<String>,
    pub start_cmd: Option<String>,
}

impl Custom {
    pub fn new() -> Self {
        Self {
            pre_hook_cmd: None,
            install_cmd: None,
            post_hook_cmd: None,
            start_cmd: None,
        }
    }

    pub fn with_pre_hook(mut self, cmd: impl Into<String>) -> Self {
        self.pre_hook_cmd = Some(cmd.into());
        self
    }

    pub fn with_install(mut self, cmd: impl Into<String>) -> Self {
        self.install_cmd = Some(cmd.into());
        self
    }

    pub fn with_post_hook(mut self, cmd: impl Into<String>) -> Self {
        self.post_hook_cmd = Some(cmd.into());
        self
    }

    pub fn with_start(mut self, cmd: impl Into<String>) -> Self {
        self.start_cmd = Some(cmd.into());
        self
    }
}

impl From<ProviderType> for Custom {
    fn from(provider: ProviderType) -> Self {
        Self {
            pre_hook_cmd: provider.get_config("pre_hook").cloned(),
            install_cmd: provider.get_config("install").cloned(),
            post_hook_cmd: provider.get_config("post_hook").cloned(),
            start_cmd: provider.get_config("start").cloned(),
        }
    }
}

impl From<Custom> for ProviderType {
    fn from(custom: Custom) -> Self {
        let mut provider = ProviderType::new("custom");
        
        if let Some(cmd) = custom.pre_hook_cmd {
            provider = provider.with_config("pre_hook", cmd);
        }
        if let Some(cmd) = custom.install_cmd {
            provider = provider.with_config("install", cmd);
        }
        if let Some(cmd) = custom.post_hook_cmd {
            provider = provider.with_config("post_hook", cmd);
        }
        if let Some(cmd) = custom.start_cmd {
            provider = provider.with_config("start", cmd);
        }
        
        provider
    }
}

impl Provider for Custom {
    fn pre_hook(&self) -> Option<Command> {
        self.pre_hook_cmd.as_ref().map(|cmd| {
            if cfg!(target_os = "linux") {
                let mut command = Command::new("sh");
                command.arg("-c").arg(cmd);
                command
            } else if cfg!(target_os = "windows") {
                let mut command = Command::new("powershell");
                command.arg("-Command").arg(cmd);
                command
            } else {
                let mut command = Command::new("sh");
                command.arg("-c").arg(cmd);
                command
            }
        })
    }

    fn install(&self) -> Option<Command> {
        self.install_cmd.as_ref().map(|cmd| {
            if cfg!(target_os = "linux") {
                let mut command = Command::new("sh");
                command.arg("-c").arg(cmd);
                command
            } else if cfg!(target_os = "windows") {
                let mut command = Command::new("powershell");
                command.arg("-Command").arg(cmd);
                command
            } else {
                let mut command = Command::new("sh");
                command.arg("-c").arg(cmd);
                command
            }
        })
    }

    fn post_hook(&self) -> Option<Command> {
        self.post_hook_cmd.as_ref().map(|cmd| {
            if cfg!(target_os = "linux") {
                let mut command = Command::new("sh");
                command.arg("-c").arg(cmd);
                command
            } else if cfg!(target_os = "windows") {
                let mut command = Command::new("powershell");
                command.arg("-Command").arg(cmd);
                command
            } else {
                let mut command = Command::new("sh");
                command.arg("-c").arg(cmd);
                command
            }
        })
    }

    fn start(&self) -> Option<Command> {
        self.start_cmd.as_ref().map(|cmd| {
            if cfg!(target_os = "linux") {
                let mut command = Command::new("sh");
                command.arg("-c").arg(cmd);
                command
            } else if cfg!(target_os = "windows") {
                let mut command = Command::new("powershell");
                command.arg("-Command").arg(cmd);
                command
            } else {
                let mut command = Command::new("sh");
                command.arg("-c").arg(cmd);
                command
            }
        })
    }
}

#[derive(Debug, Clone)]
pub struct Minecraft;

impl From<ProviderType> for Minecraft {
    fn from(_provider: ProviderType) -> Self {
        Minecraft
    }
}

impl From<Minecraft> for ProviderType {
    fn from(_minecraft: Minecraft) -> Self {
        ProviderType::new("minecraft")
    }
}

impl Provider for Minecraft {
    fn pre_hook(&self) -> Option<Command> {
        if cfg!(target_os = "linux") {
            let mut cmd = Command::new("sh");
            cmd.arg("-c")
                .arg("apt-get update && apt-get install -y libssl-dev pkg-config wget");
            Some(cmd)
        } else if cfg!(target_os = "windows") {
            let mut cmd = Command::new("powershell");
            cmd.arg("-Command").arg("choco install -y wget");
            Some(cmd)
        } else {
            None
        }
    }

    fn install(&self) -> Option<Command> {
        if cfg!(target_os = "linux") {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg(
                "apt-get install -y openjdk-17-jre-headless && update-alternatives --set java /usr/lib/jvm/java-17-openjdk-amd64/bin/java"
            );
            Some(cmd)
        } else if cfg!(target_os = "windows") {
            let mut cmd = Command::new("powershell");
            cmd.arg("-Command").arg("choco install -y openjdk");
            Some(cmd)
        } else {
            None
        }
    }

    fn post_hook(&self) -> Option<Command> {
        if cfg!(target_os = "linux") {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg(format!(
                "mkdir -p {dir} && cd {dir} && wget -O server.jar https://piston-data.mojang.com/v1/objects/84194a2f286ef7c14ed7ce0090dba59902951553/server.jar && echo 'eula=true' > eula.txt",
                dir = SERVER_DIR
            ));
            Some(cmd)
        } else if cfg!(target_os = "windows") {
            let mut cmd = Command::new("powershell");
            cmd.arg("-Command").arg(format!(
                "New-Item -ItemType Directory -Force -Path {dir}; cd {dir}; Invoke-WebRequest -Uri https://piston-data.mojang.com/v1/objects/84194a2f286ef7c14ed7ce0090dba59902951553/server.jar -OutFile server.jar; 'eula=true' | Out-File -Encoding ASCII eula.txt",
                dir = SERVER_DIR
            ));
            Some(cmd)
        } else {
            None
        }
    }

    fn start(&self) -> Option<Command> {
        if cfg!(target_os = "linux") {
            let mut cmd = Command::new("java");
            cmd.args(&["-Xmx1024M", "-Xms1024M", "-jar", "server.jar", "nogui"])
                .current_dir(SERVER_DIR);
            Some(cmd)
        } else if cfg!(target_os = "windows") {
            let mut cmd = Command::new("java");
            cmd.args(&["-Xmx1024M", "-Xms1024M", "-jar", "server.jar", "nogui"])
                .current_dir(SERVER_DIR);
            Some(cmd)
        } else {
            None
        }
    }
}

#[derive(serde::Serialize)]
struct GetState {
    start_keyword: String,
    stop_keyword: String,
}

fn get_provider(name: &str) -> Option<ProviderType> {
    match name {
        "minecraft" => Some(Minecraft.into()),
        "custom" => Some(Custom::new().into()),
        _ => None,
    }
}

// Helper function to create a custom provider with specific commands
fn create_custom_provider(
    pre_hook: Option<&str>,
    install: Option<&str>, 
    post_hook: Option<&str>,
    start: Option<&str>,
) -> ProviderType {
    let mut custom = Custom::new();
    
    if let Some(cmd) = pre_hook {
        custom = custom.with_pre_hook(cmd);
    }
    if let Some(cmd) = install {
        custom = custom.with_install(cmd);
    }
    if let Some(cmd) = post_hook {
        custom = custom.with_post_hook(cmd);
    }
    if let Some(cmd) = start {
        custom = custom.with_start(cmd);
    }
    
    custom.into()
}

