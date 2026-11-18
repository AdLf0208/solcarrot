//! `config/solcarrot.conf` through the host's checked, mod-owned config service.
//!
//! Upstream keeps its settings in a NeoForge `ModConfigSpec`; the port reads a plain
//! `key = value` file (the format [`crate::runtime`] documents) from the game's own `config/`
//! folder, writing the defaults when it is absent. The file is read once, before the worlds
//! open; a later edit needs a restart, as upstream's server config did.

use mod_api::{ConfigFileError, ConfigPath, Host};

use crate::runtime::{Config, format_config, parse_config};

/// The file's name in the game's `config/` folder, beside `options.txt`.
const CONFIG_FILE: &str = "solcarrot.conf";

/// Read `config/solcarrot.conf`, writing the upstream defaults when it is not there.
///
/// A read the service refuses is answered with the defaults, which is what upstream starts
/// with too; a failed default write only means the next start reads the defaults again.
pub fn load(host: &dyn Host) -> Config {
    let Ok(path) = ConfigPath::new(CONFIG_FILE) else {
        return Config::default();
    };
    let files = host.config_files();
    match files.read(&path) {
        Ok(bytes) => parse_config(&String::from_utf8_lossy(&bytes)),
        Err(ConfigFileError::NotFound) => {
            let config = Config::default();
            let _ = files.write(&path, format_config(&config).as_bytes());
            config
        }
        Err(_) => Config::default(),
    }
}
