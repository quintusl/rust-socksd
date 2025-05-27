use anyhow::Result;
use clap::{Arg, Command};
use rusty_socks::{Config, ProxyServer};
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    let matches = Command::new("rusty-socks")
        .version("0.1.0")
        .author("Your Name <your.email@example.com>")
        .about("A high-performance SOCKS5 and HTTP proxy server")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path")
                .default_value("config.yml"),
        )
        .arg(
            Arg::new("generate-config")
                .short('g')
                .long("generate-config")
                .value_name("FILE")
                .help("Generate a default configuration file")
                .conflicts_with("config"),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
		.num_args(0)
                .help("Enable verbose logging")
                .action(clap::ArgAction::Count),
        )
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
		.num_args(0)
                .help("Suppress all output except errors")
                .conflicts_with("verbose"),
        )
        .get_matches();

    if let Some(config_path) = matches.get_one::<String>("generate-config") {
        generate_default_config(config_path)?;
        return Ok(());
    }

    let config_path = matches.get_one::<String>("config").unwrap();
    
    let config = if std::path::Path::new(config_path).exists() {
        match Config::load_from_file(config_path) {
            Ok(config) => {
                info!("Loaded configuration from {}", config_path);
                config
            }
            Err(e) => {
                error!("Failed to load configuration from {}: {}", config_path, e);
                error!("Using default configuration. Run with --generate-config to create a template.");
                Config::default()
            }
        }
    } else {
        info!("Configuration file {} not found, using defaults", config_path);
        Config::default()
    };

    setup_logging(&config, &matches)?;

    info!("Starting Rusty SOCKS proxy server");
    info!("SOCKS5 will listen on {}:{}", config.server.bind_address, config.server.socks5_port);
    info!("HTTP proxy will listen on {}:{}", config.server.bind_address, config.server.http_port);

    let server = ProxyServer::new(config);
    
    if let Err(e) = server.start().await {
        error!("Server error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}

fn generate_default_config(path: &str) -> Result<()> {
    let config = Config::default();
    config.save_to_file(path)?;
    
    println!("Generated default configuration file: {}", path);
    println!("Edit this file to customize your proxy server settings.");
    
    Ok(())
}

fn setup_logging(config: &Config, matches: &clap::ArgMatches) -> Result<()> {
    let log_level = if matches.get_flag("quiet") {
        Level::ERROR
    } else {
        match matches.get_count("verbose") {
            0 => match config.logging.level.as_str() {
                "trace" => Level::TRACE,
                "debug" => Level::DEBUG,
                "info" => Level::INFO,
                "warn" => Level::WARN,
                "error" => Level::ERROR,
                _ => Level::INFO,
            },
            1 => Level::DEBUG,
            _ => Level::TRACE,
        }
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    Ok(())
}
