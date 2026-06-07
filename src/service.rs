use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use windows_service::service::*;
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_dispatcher;
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::RemoteDesktop::*;
use windows_sys::Win32::System::Threading::*;

static SERVICE_STOPPED: AtomicBool = AtomicBool::new(false);
static WATERMARK_PROCESS: Mutex<Option<isize>> = Mutex::new(None);

const SERVICE_NAME: &str = "NyaActivate";

windows_service::define_windows_service!(ffi_service_main, handle_service_main);

fn handle_service_main(_arguments: Vec<std::ffi::OsString>) {
    log::info!("服务启动");

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                log::info!("服务收到停止信号");
                SERVICE_STOPPED.store(true, Ordering::SeqCst);
                terminate_watermark();
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = match service_control_handler::register(SERVICE_NAME, event_handler) {
        Ok(h) => h,
        Err(e) => {
            log::error!("注册服务控制句柄失败: {e}");
            return;
        }
    };

    if let Err(e) = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    }) {
        log::error!("设置服务运行状态失败: {e}");
        return;
    }

    let exe_path = std::env::current_exe().unwrap_or_default();
    launch_watermark_in_user_session(&exe_path);

    while !SERVICE_STOPPED.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(500));
    }

    let _ = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    });

    log::info!("服务已停止");
}

fn terminate_watermark() {
    if let Ok(mut guard) = WATERMARK_PROCESS.lock() {
        if let Some(handle) = guard.take() {
            unsafe {
                TerminateProcess(handle as HANDLE, 0);
                CloseHandle(handle as HANDLE);
            }
        }
    }
}

fn launch_watermark_in_user_session(exe_path: &PathBuf) {
    unsafe {
        let session_id = WTSGetActiveConsoleSessionId();
        if session_id == 0xFFFFFFFF {
            log::error!("未找到活动用户会话, 水印将无法显示");
            return;
        }

        let mut user_token: HANDLE = std::ptr::null_mut();
        if WTSQueryUserToken(session_id, &mut user_token) == 0 {
            let err = GetLastError();
            log::error!("获取用户令牌失败: {}", err);
            eprintln!("[NyaActivate] 获取用户令牌失败 (Win32 错误码: {err})");
            return;
        }

        let cmd_str = format!("\"{}\" run", exe_path.to_string_lossy());
        let mut cmd_w: Vec<u16> = cmd_str.encode_utf16().chain(std::iter::once(0)).collect();

        // Set working directory to exe directory so config.toml is found
        let dir_w: Vec<u16> = exe_path
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("")
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let mut si = std::mem::zeroed::<STARTUPINFOW>();
        si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
        si.dwFlags = STARTF_USESHOWWINDOW;
        si.wShowWindow = 5;

        let mut pi = std::mem::zeroed::<PROCESS_INFORMATION>();

        let result = CreateProcessAsUserW(
            user_token,
            std::ptr::null(),
            cmd_w.as_mut_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            false.into(),
            0,
            std::ptr::null_mut(),
            dir_w.as_ptr(),
            &mut si,
            &mut pi,
        );

        CloseHandle(user_token);

        if result == 0 {
            let err = GetLastError();
            log::error!("在用户会话中启动进程失败: {}", err);
            // Surface error to user through stderr (visible in SCM debug)
            eprintln!("[NyaActivate] 在用户会话启动水印进程失败 (Win32 错误码: {err})");
            eprintln!("[NyaActivate] 请确保当前有用户登录。尝试直接运行 \"nya-activate.exe run\" 测试。");
            return;
        }

        CloseHandle(pi.hThread);

        // Wait briefly to detect immediate crash (exit within 2s)
        let wait_result = WaitForSingleObject(pi.hProcess, 2000);
        if wait_result == 0 {
            let mut exit_code = 0u32;
            GetExitCodeProcess(pi.hProcess, &mut exit_code);
            let msg = format!("水印进程启动后立即退出 (退出码: {exit_code})");
            log::error!("{}", msg);
            eprintln!("[NyaActivate] {msg}");
            CloseHandle(pi.hProcess);
            return;
        }

        if let Ok(mut guard) = WATERMARK_PROCESS.lock() {
            *guard = Some(pi.hProcess as isize);
        }

        log::info!("水印进程已在用户会话中启动 (PID: {})", pi.dwProcessId);
    }
}

pub fn install() -> windows_service::Result<()> {
    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )?;

    let exe_path = std::env::current_exe().unwrap();

    let service = manager.create_service(
        &ServiceInfo {
            name: SERVICE_NAME.into(),
            display_name: "NyaActivate".into(),
            service_type: ServiceType::OWN_PROCESS,
            start_type: ServiceStartType::AutoStart,
            error_control: ServiceErrorControl::Normal,
            executable_path: exe_path,
            launch_arguments: vec!["service".into(), "run".into()],
            dependencies: vec![],
            account_name: None,
            account_password: None,
        },
        ServiceAccess::CHANGE_CONFIG,
    )?;

    service.set_description("快激活 Windows 喵喵喵！！！")?;

    println!("服务已安装: {SERVICE_NAME}");
    Ok(())
}

pub fn uninstall() -> windows_service::Result<()> {
    let manager =
        ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;

    let service = manager.open_service(SERVICE_NAME, ServiceAccess::DELETE)?;
    service.delete()?;

    println!("服务已卸载: {SERVICE_NAME}");
    Ok(())
}

pub fn status() -> windows_service::Result<()> {
    let manager =
        ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;

    let service =
        manager.open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS | ServiceAccess::QUERY_CONFIG)?;
    let status = service.query_status()?;

    println!("服务名称: {SERVICE_NAME}");
    println!("状态: {:?}", status.current_state);
    println!("进程 ID: {:?}", status.process_id);
    println!("可接受控制: {:?}", status.controls_accepted);

    Ok(())
}

pub fn run_as_service() -> windows_service::Result<()> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
    Ok(())
}
