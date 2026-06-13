use std::io;
use std::path::Path;

const DEFAULT_SERVER_PORT: u16 = 25565;
const DEFAULT_LEVEL_NAME: &str = "world";
const DEFAULT_MOTD: &str = "A Minecraft Server";
const DEFAULT_MAX_PLAYERS: u32 = 20;
const DEFAULT_VIEW_DISTANCE: i32 = 1;

/// Parsed representation of a server.properties file.
///
/// Fields that are "parsed but not enforced" are stored for informational purposes
/// and to warn users about unsupported options without crashing.
#[derive(Debug, Clone)]
pub struct ServerProperties {
    pub server_port: u16,
    /// Bind address. Empty string means bind to all interfaces (0.0.0.0).
    pub server_ip: String,
    pub level_name: String,
    pub motd: String,
    pub max_players: u32,
    /// Always false — Aurelia does not implement session auth.
    pub online_mode: bool,
    /// Chunk radius (0–8). Mapped to ServerConfig::initial_chunk_radius.
    pub view_distance: i32,
    // Fields below are parsed so we can warn; not yet enforced.
    pub spawn_protection: Option<i32>,
    pub white_list: Option<bool>,
    pub allow_nether: Option<bool>,
    pub difficulty: Option<u8>,
    pub gamemode: Option<u8>,
}

impl Default for ServerProperties {
    fn default() -> Self {
        Self {
            server_port: DEFAULT_SERVER_PORT,
            server_ip: String::new(),
            level_name: DEFAULT_LEVEL_NAME.to_string(),
            motd: DEFAULT_MOTD.to_string(),
            max_players: DEFAULT_MAX_PLAYERS,
            online_mode: false,
            view_distance: DEFAULT_VIEW_DISTANCE,
            spawn_protection: None,
            white_list: None,
            allow_nether: None,
            difficulty: None,
            gamemode: None,
        }
    }
}

/// Parse a `server.properties`-format string into `ServerProperties`.
///
/// Returns the parsed properties and a list of warnings for bad or
/// unsupported values. Unknown keys are silently ignored — they never crash
/// the server.
pub fn parse_properties(content: &str) -> (ServerProperties, Vec<String>) {
    let mut props = ServerProperties::default();
    let mut warnings: Vec<String> = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();

        match key {
            "server-port" => match value.parse::<u32>() {
                Ok(p) if p <= u16::MAX as u32 => props.server_port = p as u16,
                _ => warnings.push(format!(
                    "server.properties: invalid server-port={value:?}; using default {DEFAULT_SERVER_PORT}"
                )),
            },

            "server-ip" => {
                props.server_ip = value.to_string();
            }

            "level-name" => {
                if value.trim().is_empty() {
                    warnings.push(format!(
                        "server.properties: level-name is blank; using default \"{DEFAULT_LEVEL_NAME}\""
                    ));
                } else {
                    props.level_name = value.to_string();
                }
            }

            "motd" => {
                props.motd = value.to_string();
            }

            "max-players" => match value.parse::<u32>() {
                Ok(n) => props.max_players = n,
                Err(_) => warnings.push(format!(
                    "server.properties: invalid max-players={value:?}; using default {DEFAULT_MAX_PLAYERS}"
                )),
            },

            "online-mode" => match value {
                "true" => {
                    warnings.push(
                        "server.properties: online-mode=true is not supported; \
                         Aurelia does not implement session authentication. \
                         Overriding to false."
                            .to_string(),
                    );
                    props.online_mode = false;
                }
                "false" => props.online_mode = false,
                _ => warnings.push(format!(
                    "server.properties: invalid online-mode={value:?}; using false"
                )),
            },

            "view-distance" => match value.parse::<i32>() {
                Ok(v) => {
                    let clamped = v.clamp(0, 8);
                    if clamped != v {
                        warnings.push(format!(
                            "server.properties: view-distance={v} is out of range [0,8]; \
                             clamping to {clamped}"
                        ));
                    }
                    props.view_distance = clamped;
                }
                Err(_) => warnings.push(format!(
                    "server.properties: invalid view-distance={value:?}; \
                     using default {DEFAULT_VIEW_DISTANCE}"
                )),
            },

            "spawn-protection" => match value.parse::<i32>() {
                Ok(v) => {
                    props.spawn_protection = Some(v);
                    warnings.push(
                        "server.properties: spawn-protection is parsed but not \
                         yet enforced by Aurelia."
                            .to_string(),
                    );
                }
                Err(_) => warnings.push(format!(
                    "server.properties: invalid spawn-protection={value:?}; ignoring"
                )),
            },

            "white-list" => match value {
                "true" => {
                    props.white_list = Some(true);
                    warnings.push(
                        "server.properties: white-list is parsed but not \
                         yet enforced by Aurelia."
                            .to_string(),
                    );
                }
                "false" => {
                    props.white_list = Some(false);
                }
                _ => warnings.push(format!(
                    "server.properties: invalid white-list={value:?}; ignoring"
                )),
            },

            "allow-nether" => match value {
                "true" => {
                    props.allow_nether = Some(true);
                    warnings.push(
                        "server.properties: allow-nether=true is not supported; \
                         the Nether is not implemented in Aurelia."
                            .to_string(),
                    );
                }
                "false" => {
                    props.allow_nether = Some(false);
                }
                _ => warnings.push(format!(
                    "server.properties: invalid allow-nether={value:?}; ignoring"
                )),
            },

            "difficulty" => match value.parse::<u8>() {
                Ok(d) if d <= 3 => {
                    props.difficulty = Some(d);
                    warnings.push(
                        "server.properties: difficulty is parsed but not \
                         yet enforced by Aurelia."
                            .to_string(),
                    );
                }
                _ => warnings.push(format!(
                    "server.properties: invalid difficulty={value:?}; ignoring"
                )),
            },

            "gamemode" => match value {
                "0" => {
                    props.gamemode = Some(0);
                }
                "1" => {
                    props.gamemode = Some(1);
                    warnings.push(
                        "server.properties: gamemode=1 (creative) is parsed but not \
                         yet enforced by Aurelia."
                            .to_string(),
                    );
                }
                _ => warnings.push(format!(
                    "server.properties: invalid gamemode={value:?}; ignoring"
                )),
            },

            _ => {
                // Unknown properties are silently ignored; they should not crash the server.
            }
        }
    }

    (props, warnings)
}

/// Load `server.properties` from `path`.
///
/// If the file does not exist, returns silent defaults. If the file exists
/// but cannot be read, returns defaults plus a warning.
pub fn load_server_properties(path: &Path) -> (ServerProperties, Vec<String>) {
    match std::fs::read_to_string(path) {
        Ok(content) => parse_properties(&content),
        Err(e) if e.kind() == io::ErrorKind::NotFound => (ServerProperties::default(), vec![]),
        Err(e) => {
            let warning = format!(
                "Warning: could not read {}: {e}; using defaults",
                path.display()
            );
            (ServerProperties::default(), vec![warning])
        }
    }
}

/// Default server.properties content written when no file is found.
pub const DEFAULT_PROPERTIES_CONTENT: &str = "\
# Aurelia server.properties
# Generated by Aurelia. Edit this file to configure your server.
#
# Aurelia is a clean-room Minecraft Beta 1.7.3-compatible dedicated server.
# https://github.com/Luxorium/Aurelia

#-- Network --

# IP address to bind to. Leave blank to bind to all interfaces (0.0.0.0).
server-ip=

# Port to listen on.
server-port=25565

#-- World --

# Name of the world folder (relative to the server directory).
# Aurelia auto-detects vanilla Beta 1.7.3 worlds (level.dat + region/*.mcr).
level-name=world

# Chunk radius sent to players on join (0-8).
view-distance=1

#-- Players --

# Message shown in the server list.
motd=A Minecraft Server

# Maximum number of players.
# Note: Aurelia does not enforce this limit yet.
max-players=20

# Require Mojang session authentication.
# Note: Aurelia does not implement session authentication; this must be false.
online-mode=false

#-- Options parsed but not yet enforced by Aurelia --

# Radius around the spawn point protected from building. Aurelia parses but does not enforce this.
# spawn-protection=16

# Only allow whitelisted players to join. Aurelia parses but does not enforce this.
# white-list=false

# Allow access to the Nether dimension. The Nether is not implemented in Aurelia.
# allow-nether=false

# World difficulty (0=Peaceful 1=Easy 2=Normal 3=Hard). Aurelia parses but does not enforce this.
# difficulty=1

# Default game mode (0=Survival 1=Creative). Aurelia parses but does not enforce this.
# gamemode=0
";

/// Write a default `server.properties` file to `path`.
pub fn write_default_server_properties(path: &Path) -> io::Result<()> {
    std::fs::write(path, DEFAULT_PROPERTIES_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile_helper::TempFile;

    // Minimal inline temp-file helper that avoids a test dependency.
    mod tempfile_helper {
        use std::path::{Path, PathBuf};

        pub struct TempFile {
            path: PathBuf,
        }

        impl TempFile {
            pub fn with_content(content: &str) -> std::io::Result<Self> {
                let path = std::env::temp_dir().join(format!(
                    "aurelia_props_test_{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.subsec_nanos())
                        .unwrap_or(0)
                ));
                std::fs::write(&path, content)?;
                Ok(Self { path })
            }

            pub fn path(&self) -> &Path {
                &self.path
            }
        }

        impl Drop for TempFile {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }

    #[test]
    fn default_server_properties_have_vanilla_defaults() {
        let props = ServerProperties::default();

        assert_eq!(25565, props.server_port);
        assert_eq!("", props.server_ip);
        assert_eq!("world", props.level_name);
        assert_eq!("A Minecraft Server", props.motd);
        assert_eq!(20, props.max_players);
        assert!(!props.online_mode);
        assert_eq!(1, props.view_distance);
        assert!(props.spawn_protection.is_none());
        assert!(props.white_list.is_none());
        assert!(props.allow_nether.is_none());
        assert!(props.difficulty.is_none());
        assert!(props.gamemode.is_none());
    }

    #[test]
    fn parse_properties_handles_standard_keys() {
        let content = "\
server-port=25566
server-ip=192.168.1.1
level-name=myworld
motd=Hello World
max-players=10
online-mode=false
view-distance=3
";
        let (props, warnings) = parse_properties(content);

        assert_eq!(25566, props.server_port);
        assert_eq!("192.168.1.1", props.server_ip);
        assert_eq!("myworld", props.level_name);
        assert_eq!("Hello World", props.motd);
        assert_eq!(10, props.max_players);
        assert!(!props.online_mode);
        assert_eq!(3, props.view_distance);
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    }

    #[test]
    fn parse_properties_bad_port_falls_back_to_default() {
        let (props, warnings) = parse_properties("server-port=notanumber\n");

        assert_eq!(25565, props.server_port);
        assert_eq!(1, warnings.len());
        assert!(warnings[0].contains("server-port"));
    }

    #[test]
    fn parse_properties_port_out_of_range_falls_back() {
        let (props, warnings) = parse_properties("server-port=99999\n");

        assert_eq!(25565, props.server_port);
        assert_eq!(1, warnings.len());
    }

    #[test]
    fn parse_properties_empty_level_name_falls_back() {
        let (props, warnings) = parse_properties("level-name=\n");

        assert_eq!("world", props.level_name);
        assert_eq!(1, warnings.len());
        assert!(warnings[0].contains("level-name"));
    }

    #[test]
    fn parse_properties_ignores_comments_and_blank_lines() {
        let content = "\
# This is a comment

server-port=25566
# Another comment
";
        let (props, warnings) = parse_properties(content);

        assert_eq!(25566, props.server_port);
        assert!(warnings.is_empty());
    }

    #[test]
    fn parse_properties_unknown_keys_are_silently_ignored() {
        let (props, warnings) = parse_properties("totally-unknown-key=whatever\n");

        // Unknown keys must not panic or produce warnings.
        let _ = props;
        assert!(warnings.is_empty());
    }

    #[test]
    fn parse_properties_online_mode_true_warns_and_forces_false() {
        let (props, warnings) = parse_properties("online-mode=true\n");

        assert!(!props.online_mode);
        assert_eq!(1, warnings.len());
        assert!(warnings[0].contains("online-mode"));
    }

    #[test]
    fn parse_properties_view_distance_clamped_with_warning() {
        let (props, warnings) = parse_properties("view-distance=20\n");

        assert_eq!(8, props.view_distance);
        assert_eq!(1, warnings.len());
        assert!(warnings[0].contains("view-distance"));
    }

    #[test]
    fn parse_properties_view_distance_in_range_no_warning() {
        let (props, warnings) = parse_properties("view-distance=4\n");

        assert_eq!(4, props.view_distance);
        assert!(warnings.is_empty());
    }

    #[test]
    fn parse_properties_spawn_protection_warns_not_enforced() {
        let (props, warnings) = parse_properties("spawn-protection=16\n");

        assert_eq!(Some(16), props.spawn_protection);
        assert_eq!(1, warnings.len());
        assert!(warnings[0].contains("spawn-protection"));
    }

    #[test]
    fn parse_properties_allow_nether_true_warns() {
        let (props, warnings) = parse_properties("allow-nether=true\n");

        assert_eq!(Some(true), props.allow_nether);
        assert_eq!(1, warnings.len());
        assert!(warnings[0].contains("allow-nether"));
    }

    #[test]
    fn parse_properties_allow_nether_false_no_warn() {
        let (props, warnings) = parse_properties("allow-nether=false\n");

        assert_eq!(Some(false), props.allow_nether);
        assert!(warnings.is_empty());
    }

    #[test]
    fn parse_properties_difficulty_warns_not_enforced() {
        let (props, warnings) = parse_properties("difficulty=1\n");

        assert_eq!(Some(1), props.difficulty);
        assert_eq!(1, warnings.len());
    }

    #[test]
    fn parse_properties_invalid_difficulty_warns_and_ignores() {
        let (props, warnings) = parse_properties("difficulty=99\n");

        assert!(props.difficulty.is_none());
        assert_eq!(1, warnings.len());
    }

    #[test]
    fn parse_properties_gamemode_0_no_warning() {
        let (props, warnings) = parse_properties("gamemode=0\n");

        assert_eq!(Some(0), props.gamemode);
        assert!(warnings.is_empty());
    }

    #[test]
    fn parse_properties_gamemode_1_warns() {
        let (props, warnings) = parse_properties("gamemode=1\n");

        assert_eq!(Some(1), props.gamemode);
        assert_eq!(1, warnings.len());
        assert!(warnings[0].contains("gamemode"));
    }

    #[test]
    fn parse_properties_white_list_true_warns() {
        let (props, warnings) = parse_properties("white-list=true\n");

        assert_eq!(Some(true), props.white_list);
        assert_eq!(1, warnings.len());
        assert!(warnings[0].contains("white-list"));
    }

    #[test]
    fn load_server_properties_missing_file_gives_silent_defaults() {
        let (props, warnings) =
            load_server_properties(Path::new("/nonexistent/path/server.properties"));

        assert_eq!(25565, props.server_port);
        assert_eq!("world", props.level_name);
        assert!(warnings.is_empty());
    }

    #[test]
    fn load_server_properties_valid_file_is_parsed() {
        let file = TempFile::with_content("server-port=19132\nlevel-name=testworld\n").unwrap();
        let (props, warnings) = load_server_properties(file.path());

        assert_eq!(19132, props.server_port);
        assert_eq!("testworld", props.level_name);
        assert!(warnings.is_empty());
    }

    #[test]
    fn default_properties_content_is_valid_and_parseable() {
        let (props, warnings) = parse_properties(DEFAULT_PROPERTIES_CONTENT);

        // All commented-out lines should be ignored; active defaults should round-trip.
        assert_eq!(25565, props.server_port);
        assert_eq!("world", props.level_name);
        assert_eq!("A Minecraft Server", props.motd);
        assert_eq!(20, props.max_players);
        assert!(!props.online_mode);
        assert_eq!(1, props.view_distance);
        // The template has no warnings (all values are valid).
        assert!(
            warnings.is_empty(),
            "template produced warnings: {warnings:?}"
        );
    }

    #[test]
    fn write_default_server_properties_creates_parseable_file() {
        let path = std::env::temp_dir().join("aurelia_test_write_props.properties");
        let _ = std::fs::remove_file(&path);

        write_default_server_properties(&path).unwrap();
        let (props, warnings) = load_server_properties(&path);

        let _ = std::fs::remove_file(&path);

        assert_eq!(25565, props.server_port);
        assert_eq!("world", props.level_name);
        assert!(warnings.is_empty());
    }
}
