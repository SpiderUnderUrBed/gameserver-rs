
// TODO: phase this out in the future
// This contains some default intergrations for certain types of gameservers, e.g minecraft, which does not
// belong in main as main should aim to be intergration agnostic
use serde::{self, Serialize, Deserialize};
use tokio::fs;
use std::path::Path;

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "kind")]
pub enum IntergrationCommands {
    MinecraftEnableRcon,
}

pub async fn run_intergration_commands(intergration_command: IntergrationCommands) {
    match intergration_command {
        IntergrationCommands::MinecraftEnableRcon => {
            if let Err(e) = enable_minecraft_rcon().await {
                eprintln!("Failed to enable Minecraft RCON: {}", e);
            } else {
                println!("Successfully enabled Minecraft RCON");
            }
        }
    }
}

async fn enable_minecraft_rcon() -> Result<(), Box<dyn std::error::Error>> {
    let properties_path = "server/server.properties";
    
    if !Path::new(properties_path).exists() {
        return Err(format!("server.properties not found at {}", properties_path).into());
    }
    
    let content = fs::read_to_string(properties_path).await?;
    
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let mut rcon_enabled = false;
    let mut rcon_password_exists = false;
    let mut rcon_port_exists = false;
    
    for line in lines.iter_mut() {
        let trimmed = line.trim();
        
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        
        if trimmed.starts_with("enable-rcon=") {
            *line = "enable-rcon=true".to_string();
            rcon_enabled = true;
        } else if trimmed.starts_with("rcon.password=") {
            if let Some(value) = trimmed.strip_prefix("rcon.password=") {
                if !value.trim().is_empty() {
                    rcon_password_exists = true;
                } else {
                    *line = "rcon.password=changeme".to_string();
                }
            }
        } else if trimmed.starts_with("rcon.port=") {
            rcon_port_exists = true;
        }
    }
    
    if !rcon_enabled {
        lines.push("enable-rcon=true".to_string());
    }
    
    if !rcon_password_exists {
        lines.push("rcon.password=changeme".to_string());
    }
    
    if !rcon_port_exists {
        lines.push("rcon.port=25575".to_string());
    }
    
    let new_content = lines.join("\n") + "\n";
    fs::write(properties_path, new_content).await?;
    
    
    Ok(())
}