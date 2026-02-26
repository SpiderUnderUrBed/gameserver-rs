use std::process::Command;
// use tokio::process::Command;

use std::collections::HashMap;

use serde_json::Value;


pub trait Provider {
    fn pre_hook(&self) -> Option<Command>;
    fn install(&self) -> Option<Command>;
    fn post_hook(&self) -> Option<Command>;
    fn start(&self) -> Option<Command>;
}

impl From<ProviderGame> for Custom {
    fn from(provider: ProviderGame) -> Self {
        Self {
            pre_hook_cmd: provider.get_config("pre_hook").cloned(),
            install_cmd: provider.get_config("install").cloned(),
            post_hook_cmd: provider.get_config("post_hook").cloned(),
            start_cmd: provider.get_config("start").cloned(),
        }
    }
}

impl From<Custom> for ProviderGame {
    fn from(custom: Custom) -> Self {
        let mut provider = ProviderGame::new("custom");

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


//get_provider_object
#[derive(Debug, Clone)]
pub struct Custom {
    pub pre_hook_cmd: Option<String>,
    pub install_cmd: Option<String>,
    pub post_hook_cmd: Option<String>,
    pub start_cmd: Option<String>,
}

// const SERVER_DIR: &str = "server";

#[derive(Debug, Clone)]
pub struct ProviderGame {
    pub name: String,
    pub config: std::collections::HashMap<String, Option<String>>,
}

impl ProviderGame {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            config: std::collections::HashMap::new(),
        }
    }

    pub fn with_config(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config.insert(key.into(), Some(value.into()));
        self
    }

    pub fn get_config(&self, key: &str) -> Option<&String> {
        self.config.get(key).unwrap().as_ref()
    }
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


impl Provider for ProviderGame {
    fn pre_hook(&self) -> Option<Command> {
        match self.name.as_str() {
            // "minecraft" => {
            //     let minecraft: Minecraft = self.clone().into();
            //     minecraft.pre_hook()
            // }
            "custom" => {
                let custom: Custom = self.clone().into();
                custom.pre_hook()
            }
            _ => None,
        }
    }

    fn install(&self) -> Option<Command> {
        match self.name.as_str() {
            // "minecraft" => {
            //     let minecraft: Minecraft = self.clone().into();
            //     minecraft.install()
            // }
            // "" => {}
            "custom" => {
                let custom: Custom = self.clone().into();
                custom.install()
            }
            _ => None,
        }
    }

    fn post_hook(&self) -> Option<Command> {
        match self.name.as_str() {
            // "minecraft" => {
            //     let minecraft: Minecraft = self.clone().into();
            //     minecraft.post_hook()
            // }
            "custom" => {
                let custom: Custom = self.clone().into();
                custom.post_hook()
            }
            _ => None,
        }
    }

    fn start(&self) -> Option<Command> {
        match self.name.as_str() {
            // "minecraft" => {
            //     let minecraft: Minecraft = self.clone().into();
            //     minecraft.start()
            // }
            "custom" => {
                let custom: Custom = self.clone().into();
                custom.start()
            }
            _ => None,
        }
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

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ProviderConfig {
    pub pre_hook: Option<String>,
    pub install: Option<String>,
    pub post_hook: Option<String>,
    pub start: Option<String>,
}

impl From<ProviderConfig> for ProviderGame {
    fn from(config: ProviderConfig) -> Self {
        let mut provider = ProviderGame::new("custom");
        if let Some(cmd) = config.pre_hook {
            provider = provider.with_config("pre_hook", cmd);
        }
        if let Some(cmd) = config.install {
            provider = provider.with_config("install", cmd);
        }
        if let Some(cmd) = config.post_hook {
            provider = provider.with_config("post_hook", cmd);
        }
        if let Some(cmd) = config.start {
            provider = provider.with_config("start", cmd);
        }
        provider
    }
}

impl From<ProviderGame> for ProviderConfig {
    fn from(game: ProviderGame) -> Self {
        Self {
            pre_hook: game.get_config("pre_hook").cloned(),
            install: game.get_config("install").cloned(),
            post_hook: game.get_config("post_hook").cloned(),
            start: game.get_config("start").cloned(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct BasicProvider {
    pub pre_hook: String,
    pub install: String,
    pub post_hook: String,
    pub start: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Platforms {
    pub(crate) linux: Option<ProviderConfig>,
    pub(crate) windows: Option<ProviderConfig>
}

impl Platforms {
    pub fn custom(config: ProviderConfig) -> Self {
        Self {
            linux: Some(config.clone()),
            windows: Some(config),
        }
    }
}
impl From<Custom> for Platforms {
    fn from(custom: Custom) -> Self {
        let config = ProviderConfig {
            pre_hook: custom.pre_hook_cmd,
            install: custom.install_cmd,
            post_hook: custom.post_hook_cmd,
            start: custom.start_cmd,
        };
        Platforms::custom(config)
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ProviderDbList {
    pub list: HashMap<String, Platforms>
}
