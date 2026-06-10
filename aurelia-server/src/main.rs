use aurelia_server::{parse_config, ServerBootstrap};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let smoke_test = args.iter().any(|arg| arg == "--smoke-test");
    let config = parse_config(&args).expect("invalid Aurelia server configuration");
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
