use aurelia_server::{apply_server_properties, parse_config_over, properties, ServerBootstrap};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let smoke_test = args.iter().any(|a| a == "--smoke-test");

    // Load (or generate) server.properties from the current working directory.
    let props_path = Path::new("server.properties");
    let (props, prop_warnings) = if props_path.exists() {
        properties::load_server_properties(props_path)
    } else {
        match properties::write_default_server_properties(props_path) {
            Ok(()) => eprintln!("[config] Generated default server.properties"),
            Err(e) => eprintln!("[config] Warning: could not write server.properties: {e}"),
        }
        (properties::ServerProperties::default(), vec![])
    };

    for warning in &prop_warnings {
        eprintln!("[config] {warning}");
    }

    // Build the base config from server.properties, then let CLI args override.
    let mut base = aurelia_server::ServerConfig::default_config();
    apply_server_properties(&mut base, &props);

    let config = parse_config_over(base, &args).expect("invalid Aurelia server configuration");
    let server = ServerBootstrap::new(config)
        .start()
        .expect("failed to start Aurelia");

    if smoke_test {
        drop(server);
        return;
    }

    loop {
        std::thread::park();
    }
}
