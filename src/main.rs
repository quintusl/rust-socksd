use anyhow::Result;
use clap::{Arg, Command, ArgMatches};
use rust_socksd::{Config, ProxyServer, UserConfig, HashType};
use std::io::{self, Write};
use tracing::{error, info, Level};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};
use tracing_journald;

#[tokio::main]
async fn main() -> Result<()> {
    let matches = Command::new("rust-socksd")
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
        .arg(
            Arg::new("bind")
                .short('b')
                .long("bind")
                .value_name("ADDRESS")
                .help("Bind address (can also be set via RUST_SOCKSD_BIND_ADDRESS)"),
        )
        .arg(
            Arg::new("http-port")
                .short('p')
                .long("http-port")
                .value_name("PORT")
                .help("HTTP proxy port (can also be set via RUST_SOCKSD_HTTP_PORT)"),
        )
        .arg(
            Arg::new("socks5-port")
                .short('s')
                .long("socks5-port")
                .value_name("PORT")
                .help("SOCKS5 proxy port (can also be set via RUST_SOCKSD_SOCKS5_PORT)"),
        )
        .arg(
            Arg::new("loglevel")
                .short('l')
                .long("loglevel")
                .value_name("LEVEL")
                .help("Log level: trace, debug, info, warn, error (can also be set via RUST_SOCKSD_LOG_LEVEL)"),
        )
        .subcommand(
            Command::new("validate")
                .about("Validate configuration files")
                .arg(
                    Arg::new("config")
                        .short('c')
                        .long("config")
                        .value_name("FILE")
                        .help("Configuration file to validate")
                        .default_value("config.yml"),
                )
                .arg(
                    Arg::new("user-config")
                        .long("user-config")
                        .value_name("FILE")
                        .help("User configuration file to validate"),
                ),
        )
        .subcommand(
            Command::new("user")
                .about("User management commands")
                .arg(
                    Arg::new("user-config")
                        .long("user-config")
                        .value_name("FILE")
                        .help("User configuration file path")
                        .default_value("users.yml"),
                )
                .subcommand(
                    Command::new("add")
                        .about("Add a new user")
                        .arg(
                            Arg::new("username")
                                .help("Username")
                                .required(true)
                                .index(1),
                        )
                        .arg(
                            Arg::new("password")
                                .help("Password (will prompt if not provided)")
                                .index(2),
                        )
                        .arg(
                            Arg::new("hash-type")
                                .long("hash-type")
                                .value_name("TYPE")
                                .help("Password hash type: argon2, bcrypt, scrypt")
                                .default_value("argon2"),
                        ),
                )
                .subcommand(
                    Command::new("remove")
                        .about("Remove a user")
                        .arg(
                            Arg::new("username")
                                .help("Username to remove")
                                .required(true)
                                .index(1),
                        ),
                )
                .subcommand(
                    Command::new("list")
                        .about("List all users"),
                )
                .subcommand(
                    Command::new("update")
                        .about("Update user password")
                        .arg(
                            Arg::new("username")
                                .help("Username")
                                .required(true)
                                .index(1),
                        )
                        .arg(
                            Arg::new("password")
                                .help("New password (will prompt if not provided)")
                                .index(2),
                        ),
                )
                .subcommand(
                    Command::new("enable")
                        .about("Enable/disable a user")
                        .arg(
                            Arg::new("username")
                                .help("Username")
                                .required(true)
                                .index(1),
                        )
                        .arg(
                            Arg::new("enabled")
                                .help("Enable (true) or disable (false)")
                                .required(true)
                                .index(2),
                        ),
                )
                .subcommand(
                    Command::new("init")
                        .about("Initialize a new user configuration file")
                        .arg(
                            Arg::new("hash-type")
                                .long("hash-type")
                                .value_name("TYPE")
                                .help("Default password hash type: argon2, bcrypt, scrypt")
                                .default_value("argon2"),
                        ),
                ),
        )
        .get_matches();

    if let Some(config_path) = matches.get_one::<String>("generate-config") {
        generate_default_config(config_path)?;
        return Ok(());
    }

    if let Some(validate_matches) = matches.subcommand_matches("validate") {
        handle_validate_command(validate_matches)?;
        return Ok(());
    }

    if let Some(user_matches) = matches.subcommand_matches("user") {
        handle_user_command(user_matches)?;
        return Ok(());
    }

    let config_path = matches.get_one::<String>("config").unwrap();

    let mut config = if std::path::Path::new(config_path).exists() {
        match Config::load_from_file(config_path) {
            Ok(config) => config,
            Err(_) => Config::default()
        }
    } else {
        Config::default()
    };

    // Apply CLI/environment overrides
    if let Some(bind_address) = matches.get_one::<String>("bind") {
        config.server.bind_address = bind_address.clone();
    } else if let Ok(bind_address) = std::env::var("RUST_SOCKSD_BIND_ADDRESS") {
        config.server.bind_address = bind_address;
    }
    
    if let Some(http_port) = matches.get_one::<String>("http-port") {
        if let Ok(port) = http_port.parse::<u16>() {
            config.server.http_port = port;
        }
    } else if let Ok(http_port) = std::env::var("RUST_SOCKSD_HTTP_PORT") {
        if let Ok(port) = http_port.parse::<u16>() {
            config.server.http_port = port;
        }
    }
    
    if let Some(socks5_port) = matches.get_one::<String>("socks5-port") {
        if let Ok(port) = socks5_port.parse::<u16>() {
            config.server.socks5_port = port;
        }
    } else if let Ok(socks5_port) = std::env::var("RUST_SOCKSD_SOCKS5_PORT") {
        if let Ok(port) = socks5_port.parse::<u16>() {
            config.server.socks5_port = port;
        }
    }
    
    if let Some(log_level) = matches.get_one::<String>("loglevel") {
        config.logging.level = log_level.clone();
    } else if let Ok(log_level) = std::env::var("RUST_SOCKSD_LOG_LEVEL") {
        config.logging.level = log_level;
    }

    let _guard = setup_logging(&config, &matches);

    if std::path::Path::new(config_path).exists() {
        match Config::load_from_file(config_path) {
            Ok(_) => {
                info!("Loaded configuration from {}", config_path);
            }
            Err(e) => {
                error!("Failed to load configuration from {}: {}", config_path, e);
                error!("Using default configuration. Run with --generate-config to create a template.");
            }
        }
    } else {
        info!("Configuration file {} not found, using defaults", config_path);
    };

    info!("Starting rust-socksd proxy server");
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

fn setup_logging(config: &Config, matches: &clap::ArgMatches) -> Option<WorkerGuard>{
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

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level.to_string()));

    // start collecting layers
    let mut layers = vec![];

    let registry = tracing_subscriber::registry();
    let mut guard = None;

    // Add console logging layer
    if config.logging.console {
        let console_layer = tracing_subscriber::fmt::layer::<tracing_subscriber::Registry>()
            .with_ansi(true)
            .boxed();
        layers.push(console_layer);
    }

    // Add file logging layer
    if let Some(log_file) = &config.logging.file {
        let file_appender = tracing_appender::rolling::never(
            std::path::Path::new(log_file).parent().unwrap_or(std::path::Path::new(".")),
            std::path::Path::new(log_file).file_name().unwrap()
        );
        let (non_blocking_file, file_guard) = tracing_appender::non_blocking(file_appender);
        guard = Some(file_guard);
        let file_layer = tracing_subscriber::fmt::layer::<tracing_subscriber::Registry>()
            .with_writer(non_blocking_file)
            .with_ansi(false)
            .boxed();
        layers.push(file_layer);
    }

    // Add journald logging layer
    if config.logging.journald {
        match tracing_journald::layer() {
            Ok(journald_layer) => {
                layers.push(journald_layer.boxed());
            }
            Err(e) => {
                eprintln!("Warning: Failed to initialize journald logging: {}", e);
            }
        }
    }
    registry
        .with(layers)
        .with(filter)
        .init();
    guard
}

fn handle_user_command(matches: &ArgMatches) -> Result<()> {
    let user_config_path = matches.get_one::<String>("user-config").unwrap();

    match matches.subcommand() {
        Some(("init", sub_matches)) => {
            let hash_type = parse_hash_type(sub_matches.get_one::<String>("hash-type").unwrap())?;
            init_user_config(user_config_path, hash_type)?;
        }
        Some(("add", sub_matches)) => {
            let username = sub_matches.get_one::<String>("username").unwrap();
            let password = get_password_from_args_or_prompt(sub_matches, "password")?;
            let hash_type = parse_hash_type(sub_matches.get_one::<String>("hash-type").unwrap())?;
            add_user(user_config_path, username, &password, hash_type)?;
        }
        Some(("remove", sub_matches)) => {
            let username = sub_matches.get_one::<String>("username").unwrap();
            remove_user(user_config_path, username)?;
        }
        Some(("list", _)) => {
            list_users(user_config_path)?;
        }
        Some(("update", sub_matches)) => {
            let username = sub_matches.get_one::<String>("username").unwrap();
            let password = get_password_from_args_or_prompt(sub_matches, "password")?;
            update_user_password(user_config_path, username, &password)?;
        }
        Some(("enable", sub_matches)) => {
            let username = sub_matches.get_one::<String>("username").unwrap();
            let enabled = sub_matches.get_one::<String>("enabled").unwrap().parse::<bool>()?;
            enable_user(user_config_path, username, enabled)?;
        }
        _ => {
            eprintln!("No valid user subcommand provided. Use --help for usage information.");
        }
    }

    Ok(())
}

fn parse_hash_type(hash_type_str: &str) -> Result<HashType> {
    match hash_type_str.to_lowercase().as_str() {
        "argon2" => Ok(HashType::Argon2),
        "bcrypt" => Ok(HashType::Bcrypt),
        "scrypt" => Ok(HashType::Scrypt),
        _ => Err(anyhow::anyhow!("Invalid hash type: {}. Valid options are: argon2, bcrypt, scrypt", hash_type_str)),
    }
}

fn get_password_from_args_or_prompt(matches: &ArgMatches, arg_name: &str) -> Result<String> {
    if let Some(password) = matches.get_one::<String>(arg_name) {
        Ok(password.clone())
    } else {
        print!("Enter password: ");
        io::stdout().flush()?;
        let mut password = String::new();
        io::stdin().read_line(&mut password)?;
        Ok(password.trim().to_string())
    }
}

fn init_user_config(path: &str, hash_type: HashType) -> Result<()> {
    if std::path::Path::new(path).exists() {
        return Err(anyhow::anyhow!("User config file already exists: {}", path));
    }

    let mut user_config = UserConfig::default();
    user_config.hash_type = hash_type;
    user_config.save_to_file(path)?;

    println!("Initialized user configuration file: {}", path);
    println!("Hash type: {:?}", user_config.hash_type);

    Ok(())
}

fn add_user(path: &str, username: &str, password: &str, hash_type: HashType) -> Result<()> {
    let mut user_config = if std::path::Path::new(path).exists() {
        UserConfig::load_from_file(path)?
    } else {
        let mut config = UserConfig::default();
        config.hash_type = hash_type;
        config
    };

    user_config.add_user(username.to_string(), password)?;
    user_config.save_to_file(path)?;

    println!("Added user: {}", username);

    Ok(())
}

fn remove_user(path: &str, username: &str) -> Result<()> {
    let mut user_config = UserConfig::load_from_file(path)?;
    user_config.remove_user(username)?;
    user_config.save_to_file(path)?;

    println!("Removed user: {}", username);

    Ok(())
}

fn list_users(path: &str) -> Result<()> {
    let user_config = UserConfig::load_from_file(path)?;

    println!("Hash type: {:?}", user_config.hash_type);
    println!("Users:");

    if user_config.users.is_empty() {
        println!("  No users configured");
    } else {
        for (username, user) in &user_config.users {
            let status = if user.enabled { "enabled" } else { "disabled" };
            println!("  {} ({}) - created: {}, modified: {}",
                username, status, user.created_at, user.last_modified);
        }
    }

    Ok(())
}

fn update_user_password(path: &str, username: &str, password: &str) -> Result<()> {
    let mut user_config = UserConfig::load_from_file(path)?;
    user_config.update_password(username, password)?;
    user_config.save_to_file(path)?;

    println!("Updated password for user: {}", username);

    Ok(())
}

fn enable_user(path: &str, username: &str, enabled: bool) -> Result<()> {
    let mut user_config = UserConfig::load_from_file(path)?;
    user_config.enable_user(username, enabled)?;
    user_config.save_to_file(path)?;

    let status = if enabled { "enabled" } else { "disabled" };
    println!("User {} {}", username, status);

    Ok(())
}

fn handle_validate_command(matches: &ArgMatches) -> Result<()> {
    let config_path = matches.get_one::<String>("config").unwrap();
    let user_config_path = matches.get_one::<String>("user-config");

    let mut has_errors = false;

    println!("Validating configuration files...");

    if std::path::Path::new(config_path).exists() {
        print!("Validating main config file '{}': ", config_path);
        match Config::load_from_file(config_path) {
            Ok(_) => {
                println!("✓ Valid");
            }
            Err(e) => {
                println!("✗ Invalid - {}", e);
                has_errors = true;
            }
        }
    } else {
        println!("⚠ Main config file '{}' does not exist", config_path);
    }

    if let Some(user_config_path) = user_config_path {
        if std::path::Path::new(user_config_path).exists() {
            print!("Validating user config file '{}': ", user_config_path);
            match UserConfig::load_from_file(user_config_path) {
                Ok(_) => {
                    println!("✓ Valid");
                }
                Err(e) => {
                    println!("✗ Invalid - {}", e);
                    has_errors = true;
                }
            }
        } else {
            println!("⚠ User config file '{}' does not exist", user_config_path);
        }
    }

    if has_errors {
        std::process::exit(1);
    } else {
        println!("All configuration files are valid!");
    }

    Ok(())
}
