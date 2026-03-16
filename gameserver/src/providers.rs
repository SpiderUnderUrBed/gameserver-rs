use std::fs;
use std::path::Path;
use std::process::Command;

use std::collections::HashMap;

use serde_json::Value;

pub trait Provider {
    fn set_location(
        &mut self,
        location: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn pre_hook(&self) -> Option<Command>;
    fn install(&self) -> Option<Command>;
    fn post_hook(&self) -> Option<Command>;
    fn start(&self) -> Option<Command>;
}

impl From<ProviderGame> for Custom {
    fn from(provider: ProviderGame) -> Self {
        Self {
            pre_hook_cmd: provider.get_config_commands("pre_hook").cloned(),
            install_cmd: provider.get_config_commands("install").cloned(),
            post_hook_cmd: provider.get_config_commands("post_hook").cloned(),
            start_cmd: provider.get_config_commands("start").cloned(),
            location: provider
                .get_config_commands("location")
                .cloned()
                .unwrap_or(String::new()),
            needed_paths: provider.get_config_sandboxed_paths(),
            needed_commands: provider.get_config_sandboxed_commands(),
        }
    }
}

impl From<Custom> for ProviderGame {
    fn from(custom: Custom) -> Self {
        let mut provider = ProviderGame::new("custom", custom.location);

        if let Some(cmd) = custom.pre_hook_cmd {
            provider = provider.with_config("pre_hook", Value::String(cmd));
        }
        if let Some(cmd) = custom.install_cmd {
            provider = provider.with_config("install", Value::String(cmd));
        }
        if let Some(cmd) = custom.post_hook_cmd {
            provider = provider.with_config("post_hook", Value::String(cmd));
        }
        if let Some(cmd) = custom.start_cmd {
            provider = provider.with_config("start", Value::String(cmd));
        }

        provider
    }
}

#[derive(Debug, Clone)]
pub struct Custom {
    pub pre_hook_cmd: Option<String>,
    pub install_cmd: Option<String>,
    pub post_hook_cmd: Option<String>,
    pub start_cmd: Option<String>,
    pub location: String,
    pub needed_paths: Vec<String>,
    pub needed_commands: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ProviderGame {
    pub location: String,
    pub name: String,
    pub config: std::collections::HashMap<String, Option<String>>,
    pub needed_paths: Vec<String>,
    pub needed_commands: Vec<String>,
}

impl ProviderGame {
    pub fn new(name: impl Into<String>, location: String) -> Self {
        Self {
            name: name.into(),
            location,
            config: std::collections::HashMap::new(),
            needed_paths: vec![],
            needed_commands: vec![]
        }
    }

    pub fn with_config(mut self, key: impl Into<String>, value: Value) -> Self {
        if let Value::String(string) = value {
            self.config.insert(key.into(), Some(string));
        } else if let Value::Array(array) = value {
            for binary_value in array {
                if let Value::String(binary_path) = binary_value {
                    self.needed_paths.push(binary_path);
                }
            }
        }
        self
    }
    
    // pub fn with_config(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
    //     self.config.insert(key.into(), Some(value.into()));
    //     self
    // }

    // // TODO: is this just boilerplate? or a clean method
    pub fn get_config_commands(&self, key: &str) -> Option<&String> {
        //println!("{:#?}", self.config);
        if let Some(unwrapped_key) = self.config.get(key) {
            unwrapped_key.as_ref()
        } else {
            println!("Failed to get key");
            None
        }
    }
    pub fn get_config_sandboxed_paths(&self) -> Vec<String> {
        self.needed_paths.clone()
    }
    pub fn get_config_sandboxed_commands(&self) -> Vec<String> {
        self.needed_commands.clone()
    }
}

impl Custom {
    pub fn new() -> Self {
        Self {
            pre_hook_cmd: None,
            install_cmd: None,
            post_hook_cmd: None,
            start_cmd: None,
            location: String::new(),
            needed_paths: vec![],
            needed_commands: vec![]
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
            "custom" => Custom::from(self.clone()).pre_hook(),
            _ => self.get_config_commands("pre_hook").map(|cmd| {
                let command = if cfg!(target_os = "windows") {
                    let mut c = Command::new("powershell");
                    c.arg("-Command").arg(cmd);
                    c
                } else {
                    let mut c = Command::new("sh");
                    c.arg("-c").arg(cmd);
                    c
                };
                command
            }),
        }
    }

    fn install(&self) -> Option<Command> {
        match self.name.as_str() {
            "custom" => Custom::from(self.clone()).install(),
            _ => self.get_config_commands("install").map(|cmd| {
                let command = if cfg!(target_os = "windows") {
                    let mut c = Command::new("powershell");
                    c.arg("-Command").arg(cmd);
                    c
                } else {
                    let mut c = Command::new("sh");
                    c.arg("-c").arg(cmd);
                    c
                };
                command
            }),
        }
    }

    fn post_hook(&self) -> Option<Command> {
        match self.name.as_str() {
            "custom" => Custom::from(self.clone()).post_hook(),
            _ => self.get_config_commands("post_hook").map(|cmd| {
                let command = if cfg!(target_os = "windows") {
                    let mut c = Command::new("powershell");
                    c.arg("-Command").arg(cmd);
                    c
                } else {
                    let mut c = Command::new("sh");
                    c.arg("-c").arg(cmd);
                    c
                };
                command
            }),
        }
    }

    fn start(&self) -> Option<Command> {
        if !Path::new(&self.location).exists() {
            println!("{}", self.location);
            let _ = fs::create_dir(&self.location);
        }
        match self.name.as_str() {
            "custom" => Custom::from(self.clone()).start(),
            _ => self.get_config_commands("start").map(|cmd| {
                let command = if cfg!(target_os = "windows") {
                    let mut c = Command::new("powershell");
                    c.arg("-Command").arg(cmd);
                    c
                } else {
                    let mut c = Command::new("sh");
                    c.arg("-c").arg(cmd);
                    c
                };
                command
            }),
        }
    }

    fn set_location(
        &mut self,
        location: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !Path::new(&location).exists() {
            fs::create_dir(&location)?;
        }

        let cwd = std::env::current_dir().unwrap_or_default();
        let location_stripped = location.trim_start_matches("server/");
        let resolved = cwd.join("server").join(location_stripped);

        for value in self.config.values_mut() {
            if let Some(cmd) = value {
                *cmd = cmd.replace("{{SERVERLOCATION}}", &resolved.to_string_lossy());
            }
        }

        self.location = location;
        Ok(())
    }
}

impl Provider for Custom {
    fn pre_hook(&self) -> Option<Command> {
        println!("{}", self.location);
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
            println!("location: {:#?}", self.location);
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

    fn set_location(
        &mut self,
        location: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.location = location;
        return Ok(());
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default)]
pub struct ProviderConfig {
    pub pre_hook: Option<String>,
    pub install: Option<String>,
    pub post_hook: Option<String>,
    pub start: Option<String>,
    pub location: String,
    pub needed_paths: Vec<String>,
    pub needed_commands: Vec<String>
}

impl From<ProviderConfig> for ProviderGame {
    fn from(config: ProviderConfig) -> Self {
        let mut provider = ProviderGame::new("custom", config.location.clone());
        if let Some(cmd) = config.pre_hook {
            provider = provider.with_config("pre_hook", Value::String(cmd));
        }
        if let Some(cmd) = config.install {
            provider = provider.with_config("install", Value::String(cmd));
        }
        if let Some(cmd) = config.post_hook {
            provider = provider.with_config("post_hook", Value::String(cmd));
        }
        if let Some(cmd) = config.start {
            provider = provider.with_config("start", Value::String(cmd));
        }
        provider = provider.with_config("location", Value::String(config.location));
        provider
    }
}

impl From<ProviderGame> for ProviderConfig {
    fn from(game: ProviderGame) -> Self {
        Self {
            pre_hook: game.get_config_commands("pre_hook").cloned(),
            install: game.get_config_commands("install").cloned(),
            post_hook: game.get_config_commands("post_hook").cloned(),
            start: game.get_config_commands("start").cloned(),
            location: game
                .get_config_commands("location")
                .cloned()
                .unwrap_or(String::new()),
            needed_paths: game.get_config_sandboxed_paths(),
            needed_commands: game.get_config_sandboxed_commands(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default)]
pub struct Platforms {
    pub(crate) linux: Option<ProviderConfig>,
    pub(crate) windows: Option<ProviderConfig>,
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
            location: custom.location,
            needed_paths: custom.needed_paths,
            needed_commands: custom.needed_commands,
        };
        Platforms::custom(config)
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ProviderDbList {
    pub list: HashMap<String, Platforms>,
}
