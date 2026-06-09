use clap::{Parser, Subcommand};

mod config;
mod service;
mod watermark;

const BUILD_INFO: &str = concat!(
    "v", env!("CARGO_PKG_VERSION"),
    " (", env!("GIT_HASH"),
    " build ", env!("BUILD_TIME"), ")"
);

#[derive(Parser)]
#[command(name = "NyaActivate", about = "模拟 Windows 激活水印的恶搞程序", version = BUILD_INFO)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 在前台运行水印（用于测试或由服务启动）
    Run,
    /// 管理 Windows 服务
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },
}

#[derive(Subcommand)]
enum ServiceAction {
    /// 安装 Windows 服务
    Install,
    /// 卸载 Windows 服务
    Uninstall,
    /// 查看服务状态
    Status,
    /// 服务入口（由服务控制管理器调用）
    Run,
}

fn service_err_msg(e: &windows_service::Error) -> String {
    match e {
        windows_service::Error::Winapi(io_err) => match io_err.raw_os_error() {
            Some(5) => "权限不足，请以管理员身份运行".into(),
            Some(1060) => "服务未安装".into(),
            Some(1073) => "服务已存在".into(),
            Some(code) => format!("Win32 错误码: {code}"),
            None => format!("{io_err}"),
        },
        _ => format!("{e}"),
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run => {
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
                .init();
            log::info!("NyaActivate {BUILD_INFO}");
            let exe_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .unwrap_or_default();
            let config_path = exe_dir.join("config.toml");
            let config = config::Config::load(&config_path);
            watermark::show(config);
        }
        Commands::Service { action } => match action {
            ServiceAction::Install => {
                if let Err(e) = service::install() {
                    eprintln!("安装服务失败: {}", service_err_msg(&e));
                    std::process::exit(1);
                }
            }
            ServiceAction::Uninstall => {
                if let Err(e) = service::uninstall() {
                    eprintln!("卸载服务失败: {}", service_err_msg(&e));
                    std::process::exit(1);
                }
            }
            ServiceAction::Status => {
                if let Err(e) = service::status() {
                    eprintln!("查询服务状态失败: {}", service_err_msg(&e));
                    std::process::exit(1);
                }
            }
            ServiceAction::Run => {
                if let Err(e) = service::run_as_service() {
                    eprintln!("服务运行失败: {}", service_err_msg(&e));
                    std::process::exit(1);
                }
            }
        },
    }
}
