use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::webview::NewWindowResponse;
use tauri::{Manager, State, WebviewUrl, WebviewWindowBuilder};

/// 统一版本号：由 build.rs 从根 package.json 注入（单一来源）。
const VERSION: &str = include_str!(concat!(env!("OUT_DIR"), "/version.txt"));
/// DSH++ 自带 MCA 的专属端口：与 MCA 官方默认口及独立 MCA Control
/// Center（本机 8766）错开，互不收编、互不探测。
const MCA_PORT: u16 = 18767;
/// DSH++ browser gateway (CDP-managed Chrome + shared-tab bridge).
const BROWSER_PORT: u16 = 18766;
/// Label of the embedded DSH desktop window (WebView2, loads the DSH web UI).
const DSH_WINDOW_LABEL: &str = "dsh";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
struct StoredConfig {
    dsh_host: String,
    dsh_port: u16,
    workspace: String,
    /// 显式指定的 DSH CLI 路径（bin.js / dsh.cmd / dsh.exe）。空 = 自动发现
    /// （环境变量 → PATH → npm 全局 → 旧布局）。显式配置优先于自动发现。
    dsh_cli: String,
    /// 可选远程更新源（JSON：{"version":"x.y.z","url":"https://…/DSHPlusPlus.update.exe"}）。
    /// 为空时"检查更新"只检测本地暂存的 DSHPlusPlus.update.exe。
    update_url: String,
    auto_start_dsh: bool,
    auto_open_dsh_window: bool,
    enable_mca: bool,
    enable_browser: bool,
    enable_chrome_use: bool,
    mca_image: bool,
    mca_video: bool,
    mca_audio: bool,
    mca_document: bool,
    mca_web: bool,
    mca_computer_observe: bool,
    mca_computer_act: bool,
    deepseek_base_url: String,
    deepseek_model: String,
    deepseek_secret: Option<String>,
    vision_provider: String,
    vision_base_url: String,
    vision_model: String,
    vision_api: String,
    vision_secret: Option<String>,
    enable_multimodal: bool,
}

impl Default for StoredConfig {
    fn default() -> Self {
        Self {
            dsh_host: "127.0.0.1".into(),
            dsh_port: 18760,
            workspace: std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            update_url: String::new(),
            dsh_cli: String::new(),
            auto_start_dsh: false,
            auto_open_dsh_window: true,
            enable_mca: true,
            enable_browser: true,
            enable_chrome_use: true,
            mca_image: true,
            mca_video: true,
            mca_audio: true,
            mca_document: true,
            mca_web: true,
            mca_computer_observe: true,
            mca_computer_act: true,
            deepseek_base_url: "https://api.deepseek.com".into(),
            deepseek_model: "deepseek-chat".into(),
            deepseek_secret: None,
            vision_provider: "vision-gateway".into(),
            vision_base_url: String::new(),
            vision_model: String::new(),
            vision_api: "openai-completions".into(),
            vision_secret: None,
            enable_multimodal: true,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigInput {
    dsh_host: String,
    dsh_port: u16,
    workspace: String,
    dsh_cli: String,
    update_url: String,
    auto_start_dsh: bool,
    auto_open_dsh_window: bool,
    enable_mca: bool,
    enable_browser: bool,
    enable_chrome_use: bool,
    mca_image: bool,
    mca_video: bool,
    mca_audio: bool,
    mca_document: bool,
    mca_web: bool,
    mca_computer_observe: bool,
    mca_computer_act: bool,
    deepseek_base_url: String,
    deepseek_model: String,
    deepseek_api_key: Option<String>,
    vision_provider: String,
    vision_base_url: String,
    vision_model: String,
    vision_api: String,
    vision_api_key: Option<String>,
    enable_multimodal: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfigView {
    dsh_host: String,
    dsh_port: u16,
    workspace: String,
    dsh_cli: String,
    update_url: String,
    auto_start_dsh: bool,
    auto_open_dsh_window: bool,
    enable_mca: bool,
    enable_browser: bool,
    enable_chrome_use: bool,
    mca_image: bool,
    mca_video: bool,
    mca_audio: bool,
    mca_document: bool,
    mca_web: bool,
    mca_computer_observe: bool,
    mca_computer_act: bool,
    deepseek_base_url: String,
    deepseek_model: String,
    has_deepseek_key: bool,
    vision_provider: String,
    vision_base_url: String,
    vision_model: String,
    vision_api: String,
    has_vision_key: bool,
    enable_multimodal: bool,
}

impl From<&StoredConfig> for ConfigView {
    fn from(value: &StoredConfig) -> Self {
        Self {
            dsh_host: value.dsh_host.clone(),
            dsh_port: value.dsh_port,
            workspace: value.workspace.clone(),
            dsh_cli: value.dsh_cli.clone(),
            update_url: value.update_url.clone(),
            auto_start_dsh: value.auto_start_dsh,
            auto_open_dsh_window: value.auto_open_dsh_window,
            enable_mca: value.enable_mca,
            enable_browser: value.enable_browser,
            enable_chrome_use: value.enable_chrome_use,
            mca_image: value.mca_image,
            mca_video: value.mca_video,
            mca_audio: value.mca_audio,
            mca_document: value.mca_document,
            mca_web: value.mca_web,
            mca_computer_observe: value.mca_computer_observe,
            mca_computer_act: value.mca_computer_act,
            deepseek_base_url: value.deepseek_base_url.clone(),
            deepseek_model: value.deepseek_model.clone(),
            has_deepseek_key: value.deepseek_secret.is_some(),
            vision_provider: value.vision_provider.clone(),
            vision_base_url: value.vision_base_url.clone(),
            vision_model: value.vision_model.clone(),
            vision_api: value.vision_api.clone(),
            has_vision_key: value.vision_secret.is_some(),
            enable_multimodal: value.enable_multimodal,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeInfo {
    portable: bool,
    data_root: String,
    dsh_home: Option<String>,
    dsh_cli: Option<String>,
    node_binary: Option<String>,
    mca_binary: Option<String>,
    browser_gateway: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
enum ServiceState {
    Stopped,
    Starting,
    Running,
    Error,
}

struct ManagedChild {
    child: Option<Child>,
    #[cfg(target_os = "windows")]
    job_handle: Option<isize>,
    state: ServiceState,
    message: String,
}

impl ManagedChild {
    fn stopped(message: &str) -> Self {
        Self {
            child: None,
            #[cfg(target_os = "windows")]
            job_handle: None,
            state: ServiceState::Stopped,
            message: message.into(),
        }
    }

    fn stop(&mut self) {
        #[cfg(target_os = "windows")]
        if let Some(handle) = self.job_handle.take() {
            unsafe {
                windows_sys::Win32::Foundation::CloseHandle(handle as _);
            }
        }
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.state = ServiceState::Stopped;
        self.message = "已停止".into();
    }

    fn refresh(&mut self, host: &str, port: u16) {
        if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    self.child = None;
                    #[cfg(target_os = "windows")]
                    if let Some(handle) = self.job_handle.take() {
                        unsafe {
                            windows_sys::Win32::Foundation::CloseHandle(handle as _);
                        }
                    }
                    if port_open(host, port) {
                        self.state = ServiceState::Running;
                        self.message = format!("已连接到正在监听的服务（启动进程退出：{status}）");
                    } else {
                        self.state = ServiceState::Error;
                        self.message = format!("进程已退出（{status}）");
                    }
                }
                Ok(None) => {
                    if port_open(host, port) {
                        self.state = ServiceState::Running;
                        self.message = format!("正在监听 {host}:{port}");
                    } else if !matches!(self.state, ServiceState::Error) {
                        self.state = ServiceState::Starting;
                        self.message = "进程已创建，等待服务就绪".into();
                    }
                }
                Err(error) => {
                    self.state = ServiceState::Error;
                    self.message = error.to_string();
                }
            }
        } else if matches!(self.state, ServiceState::Running) {
            if !port_open(host, port) {
                self.state = ServiceState::Stopped;
                self.message = "服务连接已断开".into();
            }
        } else if !matches!(self.state, ServiceState::Starting | ServiceState::Error) {
            self.state = ServiceState::Stopped;
        }
    }
}

struct AppState {
    config: Mutex<StoredConfig>,
    dsh: Mutex<ManagedChild>,
    mca: Mutex<ManagedChild>,
    browser: Mutex<ManagedChild>,
    /// chromeUse 扩展安装状态扫描缓存（时间戳 + 结果，3 秒 TTL）。
    extension_status: Mutex<Option<(Instant, ChromeExtensionView)>>,
    runtime: RuntimePaths,
}

impl Drop for AppState {
    fn drop(&mut self) {
        unregister_chrome_native_host();
        if let Ok(dsh) = self.dsh.get_mut() {
            dsh.stop();
        }
        if let Ok(mca) = self.mca.get_mut() {
            mca.stop();
        }
        if let Ok(browser) = self.browser.get_mut() {
            browser.stop();
        }
    }
}

#[cfg(target_os = "windows")]
fn unregister_chrome_native_host() {
    // Chrome 与 Edge 两个键都注销（指向同一份 host manifest）。
    for root in [
        r"HKCU\Software\Google\Chrome\NativeMessagingHosts\com.dshplusplus.browser",
        r"HKCU\Software\Microsoft\Edge\NativeMessagingHosts\com.dshplusplus.browser",
    ] {
        let mut command = Command::new("reg");
        command.args(["delete", root, "/f"]);
        hide_console(&mut command);
        let _ = command.output();
    }
}

#[cfg(not(target_os = "windows"))]
fn unregister_chrome_native_host() {}

#[derive(Debug, Clone)]
struct RuntimePaths {
    portable: bool,
    data_root: PathBuf,
    /// dsh 数据目录。`None` = 使用 dsh 标准 home（~/.dsh），与用户自装
    /// dsh 共享数据；`Some` = 显式指定（便携模式或 DSHPLUSPLUS_DSH_HOME）。
    dsh_home: Option<PathBuf>,
    node: Option<PathBuf>,
    dsh_cli: Option<PathBuf>,
    /// exe 自带的 DSH++ 插件目录（<root>/plugins/@dshplusplus），
    /// materialize 时复制到 home profile。完整包不再内置 DSH 本体。
    plugins_dir: Option<PathBuf>,
    mca: Option<PathBuf>,
    browser_gateway: Option<PathBuf>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppSnapshot {
    version: &'static str,
    config: ConfigView,
    runtime: RuntimeInfo,
    dsh_state: ServiceState,
    dsh_url: String,
    dsh_pid: Option<u32>,
    dsh_message: String,
    mca_state: ServiceState,
    mca_url: Option<String>,
    mca_pid: Option<u32>,
    mca_message: String,
    mca_route: Option<McaRouteView>,
    mca_providers: Vec<McaProviderView>,
    browser_state: ServiceState,
    browser_pid: Option<u32>,
    browser_message: String,
    /// chromeUse 扩展在 Chrome / Edge 的安装四态与桥连接状态。
    chrome_extension: ChromeExtensionView,
}

/// MCA deepseek-tui 路由的实时能力与健康视图（供 UI 动态化能力开关）。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct McaRouteView {
    agent_id: String,
    route_available: bool,
    /// 当前已启用的能力（image/video/audio/document/web/computer.observe/computer.act）。
    capabilities: Vec<String>,
    /// 路由声明可用的能力全集。
    available_capabilities: Vec<String>,
    /// 电脑 Provider 是否启用（false 时 computer.* 实际不可用）。
    computer_provider_enabled: bool,
    /// 路由健康总评（ready / not_checked / unavailable / disabled）。
    health: String,
    /// 首个阻塞层的具体原因（无阻塞层时为空）。
    health_detail: String,
}

/// MCA 单个 Provider 的工具级健康（供 UI 能力卡片展示）。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct McaProviderView {
    provider_id: String,
    enabled: bool,
    available: bool,
    detail: String,
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

const DSH_CLI_RELATIVE_PATHS: &[&str] = &[
    "apps/cli/lib/bin.js",
    "lib/bin.js",
    "@deepseek-ai/dsh/lib/bin.js",
    "node_modules/@deepseek-ai/dsh/lib/bin.js",
    "runtime/dsh/node_modules/@deepseek-ai/dsh/lib/bin.js",
    "bin.js",
    "dsh.cmd",
    "dsh.exe",
];

/// CLI 文件是否能在本平台被直接启动。
///
/// Windows 上 npm 全局安装会在 bin 目录同时生成无扩展名的 sh shim
/// （`dsh`，供 Git Bash 使用）与 `dsh.ps1`；二者都无法被 CreateProcess
/// 直接执行（报 os error 193），必须排除，只接受 node 脚本、cmd/bat
/// shim 与原生 exe。
fn is_executable_cli_file(path: &Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        path.extension().is_some_and(|ext| {
            ext.eq_ignore_ascii_case("js")
                || ext.eq_ignore_ascii_case("mjs")
                || ext.eq_ignore_ascii_case("cjs")
                || ext.eq_ignore_ascii_case("cmd")
                || ext.eq_ignore_ascii_case("bat")
                || ext.eq_ignore_ascii_case("exe")
        })
    }
    #[cfg(not(target_os = "windows"))]
    {
        match path.extension() {
            Some(ext) => !ext.eq_ignore_ascii_case("ps1"),
            None => true,
        }
    }
}

/// Accept a CLI file or a directory at any common DSH install/source level.
fn resolve_dsh_cli_candidate(input: &Path) -> Option<PathBuf> {
    if input.is_file() {
        if !is_executable_cli_file(input) {
            return None;
        }
        return Some(input.to_path_buf());
    }
    if !input.is_dir() {
        return None;
    }
    DSH_CLI_RELATIVE_PATHS
        .iter()
        .map(|relative| input.join(relative))
        .find(|candidate| candidate.is_file())
}

/// Search a small, deterministic set of ancestors and conventional checkout names.
fn find_dsh_near_paths(starts: &[PathBuf]) -> Option<PathBuf> {
    const CHECKOUT_NAMES: &[&str] = &["DeepseekHarness", "DeepSeekHarness", "deepseek-harness"];
    let mut visited = std::collections::HashSet::new();
    for start in starts {
        for ancestor in start.ancestors() {
            let ancestor = ancestor.to_path_buf();
            if !visited.insert(ancestor.clone()) {
                continue;
            }
            if let Some(cli) = resolve_dsh_cli_candidate(&ancestor) {
                return Some(cli);
            }
            for name in CHECKOUT_NAMES {
                if let Some(cli) = resolve_dsh_cli_candidate(&ancestor.join(name)) {
                    return Some(cli);
                }
            }
        }
    }
    None
}

fn find_dsh_source_checkout() -> Option<PathBuf> {
    let mut starts = Vec::new();
    if let Ok(current) = std::env::current_dir() {
        starts.push(current);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            starts.push(parent.to_path_buf());
        }
    }
    if let Some(profile) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
        starts.extend([
            profile.clone(),
            profile.join("source/repos"),
            profile.join("Documents/GitHub"),
            profile.join("Projects"),
            profile.join("dev"),
        ]);
    }
    find_dsh_near_paths(&starts)
}

fn find_project_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|path| {
            path.join("runtime/dsh/package.json").is_file()
                && path.join("packages/bundle-plus/package.json").is_file()
        })
        .map(Path::to_path_buf)
}

fn discover_runtime() -> Result<RuntimePaths, String> {
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let exe_dir = exe.parent().ok_or("无法解析程序目录")?.to_path_buf();
    let sibling_node = exe_dir.join("runtime/node/node.exe");
    let sibling_dsh = exe_dir.join("runtime/dsh/node_modules/@deepseek-ai/dsh/lib/bin.js");
    let sibling_mca = exe_dir.join("runtime/mca/mca-runtime.exe");
    let sibling_browser = exe_dir.join("runtime/browser/gateway.js");
    // 本机发现的 DSH：完整包不再内置 DSH 本体。exe 自带的是 @dshplusplus
    // 插件（plugins/），DSH 本体由用户安装（PATH / npm 全局 / 旧完整包布局）。
    let portable = sibling_node.is_file() && (sibling_dsh.is_file() || exe_dir.join("plugins/@dshplusplus").is_dir());

    let project = find_project_root(&exe_dir)
        .or_else(|| find_project_root(Path::new(env!("CARGO_MANIFEST_DIR"))));
    let project_node = PathBuf::from(r"C:\Program Files\nodejs\node.exe");
    let project_dsh = project
        .as_ref()
        .map(|root| root.join("runtime/dsh/node_modules/@deepseek-ai/dsh/lib/bin.js"));
    let project_mca = project.as_ref().map(|_| {
        PathBuf::from(r"E:\MCA — Multi-Modal Content Adapter\build\sidecar\mca-runtime.exe")
    });
    let project_browser = project
        .as_ref()
        .map(|root| root.join("packages/browser-gateway/lib/index.js"));

    let node = std::env::var_os("DSHPLUSPLUS_NODE")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| sibling_node.is_file().then_some(sibling_node))
        .or_else(|| project_node.is_file().then_some(project_node));
    // DSH CLI 发现：显式环境变量 → PATH → npm/pnpm 全局 → 相邻源码仓库 → 旧布局。
    let dsh_cli = std::env::var_os("DSHPLUSPLUS_DSH_CLI")
        .and_then(|value| resolve_dsh_cli_candidate(&PathBuf::from(value)))
        .or_else(|| find_dsh_on_path())
        .or_else(|| find_dsh_npm_global())
        .or_else(find_dsh_source_checkout)
        .or_else(|| sibling_dsh.is_file().then_some(sibling_dsh))
        .or_else(|| project_dsh.filter(|path| path.is_file()));
    let data_root = std::env::var_os("DSHPLUSPLUS_DATA_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if portable {
                exe_dir.join(".portable")
            } else {
                project.clone().unwrap_or_else(|| exe_dir.clone()).join(".tmp/desktop-data")
            }
        });
    fs::create_dir_all(&data_root).map_err(|error| format!("无法创建数据目录：{error}"))?;
    // 插件目录：在线更新的副本（data_root/plugins，可写）优先，回退 exe 自带。
    let plugins_dir = {
        let updated = data_root.join("plugins/@dshplusplus");
        if updated.is_dir() {
            updated
        } else {
            exe_dir.join("plugins/@dshplusplus")
        }
    };
    let plugins_dir = plugins_dir.is_dir().then_some(plugins_dir);
    // MCA：在线更新的副本（data_root/mca，可写）优先，回退内置/环境变量。
    let mca = {
        let updated = data_root.join("mca/mca-runtime.exe");
        if updated.is_file() {
            Some(updated)
        } else {
            std::env::var_os("DSHPLUSPLUS_MCA")
                .map(PathBuf::from)
                .filter(|path| path.is_file())
                .or_else(|| sibling_mca.is_file().then_some(sibling_mca))
                .or_else(|| project_mca.filter(|path| path.is_file()))
        }
    };
    let browser_gateway = std::env::var_os("DSHPLUSPLUS_BROWSER")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| sibling_browser.is_file().then_some(sibling_browser))
        .or_else(|| project_browser.filter(|path| path.is_file()));

    // dsh 数据目录：默认与用户自装 dsh 共享标准 home（~/.dsh），卸载/更新
    // dsh++ 不影响 dsh 数据；仅显式要求便携时才放回包内数据目录。
    let dsh_home = std::env::var_os("DSHPLUSPLUS_DSH_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            if std::env::var_os("DSHPLUSPLUS_PORTABLE").is_some() {
                Some(data_root.join("dsh-home"))
            } else {
                None
            }
        });
    Ok(RuntimePaths {
        portable,
        data_root,
        dsh_home,
        node,
        dsh_cli,
        plugins_dir,
        mca,
        browser_gateway,
    })
}

/// 在 PATH 中查找 `dsh`（dsh.exe / dsh.cmd / bin.js）。
///
/// `where dsh` 会按目录顺序列出所有命中项；npm 全局安装时排在最前的
/// 是无扩展名的 sh shim（Git Bash 用），无法在 Windows 直接执行，须
/// 逐行筛选，取第一个可执行的 CLI 形态。
fn find_dsh_on_path() -> Option<PathBuf> {
    let output = std::process::Command::new("where")
        .arg("dsh")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .find_map(|line| resolve_dsh_cli_candidate(Path::new(line)))
}

/// 查找 npm/pnpm 全局安装的 dsh；不依赖 GUI 进程继承到最新 PATH。
fn find_dsh_npm_global() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(base) = std::env::var_os("APPDATA").map(PathBuf::from) {
        candidates.push(base.join("npm"));
    }
    if let Some(base) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        candidates.push(base.join("pnpm"));
    }
    if let Some(base) = std::env::var_os("PNPM_HOME").map(PathBuf::from) {
        candidates.push(base);
    }
    for candidate in candidates {
        if let Some(cli) = resolve_dsh_cli_candidate(&candidate) {
            return Some(cli);
        }
    }
    let output = Command::new("npm").args(["root", "--global"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let root = String::from_utf8_lossy(&output.stdout);
    resolve_dsh_cli_candidate(Path::new(root.trim()))
}

/// dsh 的标准数据目录：不设 `DSH_HOME` 时 dsh 使用 `~/.dsh`
/// （与 `@deepseek-ai/dsh-home-paths` 的 `defaultDshHome` 一致）。
fn default_dsh_home() -> Option<PathBuf> {
    let base = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    Some(PathBuf::from(base).join(".dsh"))
}

/// 本次启动实际使用的 dsh home：显式指定 > 便携模式 > 标准 home > 兜底。
fn effective_dsh_home(runtime: &RuntimePaths) -> PathBuf {
    runtime
        .dsh_home
        .clone()
        .or_else(default_dsh_home)
        .unwrap_or_else(|| runtime.data_root.join("dsh-home"))
}

/// 旧便携 home → 标准 home 的一次性数据迁移（幂等，可反复执行）：
/// 1. 复制标准 home 缺失的会话日志目录（按 session id 判重）；
/// 2. 合并 workspace 注册表（按 path 判重，目标文件缺失则整体复制）；
/// 3. 首启补全 settings.yaml 与匿名身份（目标缺失时继承）。
///
/// 仅在本次启动使用标准 home（未显式指定 DSHPLUSPLUS_DSH_HOME /
/// DSHPLUSPLUS_PORTABLE）且旧便携 home 存在时执行。这样卸载、更新 dsh++
/// 不会影响 dsh 数据，而升级前已在 .portable 里产生的会话仍能找回。
fn migrate_portable_home_data(runtime: &RuntimePaths) -> Result<(), String> {
    if runtime.dsh_home.is_some() {
        return Ok(()); // 显式指定了 home（含便携模式）：不迁移
    }
    let Some(target) = default_dsh_home() else {
        return Ok(());
    };
    let legacy = runtime.data_root.join("dsh-home");
    if !legacy.is_dir() {
        return Ok(()); // 没有旧便携 home
    }
    // 1) 会话日志：复制缺失的会话目录（保持 sessions/<project>/<session> 布局）
    let legacy_sessions = legacy.join("sessions");
    let target_sessions = target.join("sessions");
    if legacy_sessions.is_dir() {
        let mut copied = 0usize;
        for project in fs::read_dir(&legacy_sessions).map_err(|error| error.to_string())? {
            let project = project.map_err(|error| error.to_string())?;
            if !project.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let project_name = project.file_name();
            for session in fs::read_dir(project.path()).map_err(|error| error.to_string())? {
                let session = session.map_err(|error| error.to_string())?;
                if !session.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }
                let dest = target_sessions.join(&project_name).join(session.file_name());
                if dest.join("session.jsonl.zstd").is_file()
                    || dest.join("session.jsonl").is_file()
                {
                    continue; // 目标已有该会话
                }
                copy_directory(&session.path(), &dest)?;
                copied += 1;
            }
        }
        if copied > 0 {
            eprintln!(
                "[dshplusplus] 已将 {copied} 个会话从 {} 迁移到 {}",
                path_string(&legacy),
                path_string(&target)
            );
        }
    }
    // 2) workspace 注册表：按 path 合并缺失记录
    merge_workspace_registry(
        &legacy.join("storages/workspace.json"),
        &target.join("storages/workspace.json"),
    )?;
    // 3) 首启补全：settings.yaml 与匿名身份在目标缺失时继承
    for name in ["settings.yaml", ".anonymous-user-id"] {
        let src = legacy.join(name);
        let dst = target.join(name);
        if src.is_file() && !dst.exists() {
            fs::copy(&src, &dst)
                .map_err(|error| format!("无法复制 {name}：{error}"))?;
        }
    }
    Ok(())
}

/// 合并 workspace.json：把 legacy 中目标没有的 workspace 记录（按 path 判重）
/// 并入目标注册表，并把新 id 追加到全局顺序。目标文件缺失时整体复制。
fn merge_workspace_registry(legacy: &Path, target: &Path) -> Result<(), String> {
    if !legacy.is_file() {
        return Ok(());
    }
    let legacy_value: serde_json::Value = serde_json::from_slice(
        &fs::read(legacy).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    if !target.is_file() {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::copy(legacy, target).map_err(|error| error.to_string())?;
        eprintln!(
            "[dshplusplus] workspace 注册表已复制到 {}",
            path_string(target)
        );
        return Ok(());
    }
    let mut target_value: serde_json::Value = serde_json::from_slice(
        &fs::read(target).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let Some(legacy_ws) = legacy_value
        .get("tables")
        .and_then(|tables| tables.get("workspaces"))
        .and_then(serde_json::Value::as_object)
    else {
        return Ok(());
    };
    let Some(target_ws) = target_value
        .get_mut("tables")
        .and_then(|tables| tables.get_mut("workspaces"))
        .and_then(serde_json::Value::as_object_mut)
    else {
        return Ok(());
    };
    let mut target_paths: std::collections::HashSet<String> = target_ws
        .values()
        .filter_map(|record| record.get("path").and_then(serde_json::Value::as_str))
        .map(String::from)
        .collect();
    let mut merged = false;
    let mut inserted_ids: Vec<String> = Vec::new();
    for (id, record) in legacy_ws {
        if target_ws.contains_key(id) {
            continue;
        }
        let Some(path) = record.get("path").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if target_paths.contains(path) {
            continue;
        }
        target_ws.insert(id.clone(), record.clone());
        target_paths.insert(path.to_string());
        inserted_ids.push(id.clone());
        merged = true;
    }
    if !merged {
        return Ok(());
    }
    if let Some(ids) = target_value
        .get_mut("global")
        .and_then(|global| global.get_mut("workspaceIds"))
        .and_then(serde_json::Value::as_array_mut)
    {
        for id in inserted_ids {
            if !ids.iter().any(|value| value.as_str() == Some(id.as_str())) {
                ids.push(serde_json::Value::String(id));
            }
        }
    }
    write_atomic(
        target,
        serde_json::to_vec_pretty(&target_value)
            .map_err(|error| error.to_string())?
            .as_slice(),
    )?;
    eprintln!(
        "[dshplusplus] workspace 注册表已合并到 {}",
        path_string(target)
    );
    Ok(())
}

fn config_path(runtime: &RuntimePaths) -> PathBuf {
    runtime.data_root.join("dshplusplus.json")
}

fn load_config(runtime: &RuntimePaths) -> StoredConfig {
    let mut config: StoredConfig = fs::read_to_string(config_path(runtime))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    // 电脑能力属于开箱即用范围：历史配置保存的 false 一律归一化为 true，
    // 避免旧配置落进“开关禁用 ↔ Provider 未启用”的死锁。
    config.mca_computer_observe = true;
    config.mca_computer_act = true;
    config
}

fn write_atomic(path: &Path, content: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let temp = path.with_extension("tmp");
    fs::write(&temp, content).map_err(|error| error.to_string())?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    fs::rename(temp, path).map_err(|error| error.to_string())
}

#[cfg(target_os = "windows")]
fn protect_secret(secret: &str) -> Result<String, String> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    let bytes = secret.as_bytes();
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let ok = unsafe {
        CryptProtectData(
            &input,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err("Windows DPAPI 加密失败".into());
    }
    let result = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) };
    let encoded = BASE64.encode(result);
    unsafe {
        LocalFree(output.pbData.cast());
    }
    Ok(encoded)
}

#[cfg(target_os = "windows")]
fn unprotect_secret(encoded: &str) -> Result<String, String> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    let bytes = BASE64.decode(encoded).map_err(|_| "保存的密钥格式无效")?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err("Windows DPAPI 解密失败；密钥可能来自另一台电脑".into());
    }
    let result = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) };
    let text = String::from_utf8(result.to_vec()).map_err(|_| "解密后的密钥不是 UTF-8")?;
    unsafe {
        LocalFree(output.pbData.cast());
    }
    Ok(text)
}

#[cfg(not(target_os = "windows"))]
fn protect_secret(secret: &str) -> Result<String, String> {
    Ok(BASE64.encode(secret))
}
#[cfg(not(target_os = "windows"))]
fn unprotect_secret(encoded: &str) -> Result<String, String> {
    String::from_utf8(BASE64.decode(encoded).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}

fn validate_url(value: &str, field: &str, allow_empty: bool) -> Result<(), String> {
    if value.is_empty() && allow_empty {
        return Ok(());
    }
    if value.starts_with("https://")
        || value.starts_with("http://127.0.0.1")
        || value.starts_with("http://localhost")
    {
        return Ok(());
    }
    Err(format!("{field} 必须使用 HTTPS；只有本机地址允许 HTTP"))
}

fn validate_config(input: &ConfigInput) -> Result<(), String> {
    if input.dsh_host != "127.0.0.1" && input.dsh_host != "localhost" {
        return Err("DSH 监听地址只能是 127.0.0.1 或 localhost".into());
    }
    if input.dsh_port < 1024 {
        return Err("DSH 端口必须不小于 1024".into());
    }
    if input.deepseek_model.is_empty() {
        return Err("DeepSeek 模型不能为空".into());
    }
    if input.mca_computer_act && !input.mca_computer_observe {
        return Err("启用“操作电脑”时必须同时启用“观察电脑”".into());
    }
    validate_url(&input.deepseek_base_url, "DeepSeek Base URL", false)?;
    if input.enable_multimodal {
        if input.vision_provider.is_empty()
            || !input
                .vision_provider
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        {
            return Err("多模态 Provider ID 只能包含字母、数字、连字符和下划线".into());
        }
        // 视觉模型与 Base URL 允许留空：开关默认开启，配置不完整时
        // 插件保持禁用（见 materialize_dsh_config），由 UI 提示补充。
        validate_url(&input.vision_base_url, "多模态 Base URL", true)?;
    }
    if !input.workspace.is_empty() && !Path::new(&input.workspace).is_dir() {
        return Err("默认工作目录不存在".into());
    }
    if !input.dsh_cli.trim().is_empty()
        && resolve_dsh_cli_candidate(Path::new(input.dsh_cli.trim())).is_none()
    {
        return Err("所选位置中未找到 DSH CLI；请选择 DeepSeekHarness 仓库目录、安装目录或 bin.js/dsh.cmd/dsh.exe".into());
    }
    Ok(())
}

fn yaml_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// dsh-llm-deepseek 的内置默认主模型配置（用户从未显式配置 llm-deepseek 段
/// 时使用）：apiKeyEnv=DEEPSEEK_API_KEY、baseURL=api.deepseek.com、默认模型
/// 目录（deepseek-v4-flash / deepseek-v4-pro，容量与 dsh 默认一致）。
fn default_deepseek_primary_config() -> serde_yaml::Mapping {
    let mut config = serde_yaml::Mapping::new();
    config.insert(
        serde_yaml::Value::String("apiKeyEnv".into()),
        serde_yaml::Value::String("DEEPSEEK_API_KEY".into()),
    );
    config.insert(
        serde_yaml::Value::String("baseURL".into()),
        serde_yaml::Value::String("https://api.deepseek.com".into()),
    );
    let models: Vec<serde_yaml::Value> = [("deepseek-v4-flash", "DeepSeek-V4-Flash"), ("deepseek-v4-pro", "DeepSeek-V4-Pro")]
        .into_iter()
        .map(|(id, name)| {
            let mut model = serde_yaml::Mapping::new();
            model.insert(
                serde_yaml::Value::String("id".into()),
                serde_yaml::Value::String(id.into()),
            );
            model.insert(
                serde_yaml::Value::String("name".into()),
                serde_yaml::Value::String(name.into()),
            );
            model.insert(
                serde_yaml::Value::String("contextWindow".into()),
                serde_yaml::Value::Number(1_000_000u64.into()),
            );
            model.insert(
                serde_yaml::Value::String("maxTokens".into()),
                serde_yaml::Value::Number(256_000u64.into()),
            );
            serde_yaml::Value::Mapping(model)
        })
        .collect();
    config.insert(
        serde_yaml::Value::String("models".into()),
        serde_yaml::Value::Sequence(models),
    );
    config
}

fn copy_directory(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    for entry in fs::read_dir(source)
        .map_err(|error| format!("无法读取 {}：{error}", path_string(source)))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        let from = entry.path();
        let to = destination.join(entry.file_name());
        let metadata = fs::metadata(&from).map_err(|error| error.to_string())?;
        if metadata.is_dir() {
            copy_directory(&from, &to)?;
        } else if metadata.is_file() {
            fs::copy(&from, &to)
                .map_err(|error| format!("无法复制 {}：{error}", path_string(&from)))?;
        }
    }
    Ok(())
}

fn materialize_profile_plugins(
    runtime: &RuntimePaths,
    cli: Option<&Path>,
    profile: &Path,
) -> Result<(), String> {
    // 插件来源：exe 自带 plugins/@dshplusplus（新布局）→ 生效 CLI 的
    // node_modules（旧布局兼容）→ 无（跳过，配置仍写入，等插件就绪）。
    let source_scope = runtime
        .plugins_dir
        .clone()
        .or_else(|| {
            cli.and_then(|cli| {
                let node_modules = cli
                    .ancestors()
                    .find(|path| path.file_name().is_some_and(|name| name == "node_modules"))?;
                let scope = node_modules.join("@dshplusplus");
                scope.is_dir().then_some(scope)
            })
        })
        .filter(|scope| scope.is_dir());
    let Some(source_scope) = source_scope else {
        eprintln!("[dshplusplus] 未找到 @dshplusplus 插件来源（plugins/ 或旧布局），跳过插件复制");
        return Ok(());
    };
    let destination_scope = profile.join("node_modules/@dshplusplus");
    for package in [
        "multimodal",
        "multimodal-llm",
        "multimodal-router",
        "tool-media-inspect",
        "bundle-plus",
    ] {
        let source = source_scope.join(package);
        if !source.is_dir() {
            eprintln!("[dshplusplus] 缺少 @dshplusplus/{package}，跳过");
            continue;
        }
        copy_directory(&source, &destination_scope.join(package))?;
    }
    Ok(())
}

fn materialize_dsh_config(
    runtime: &RuntimePaths,
    config: &StoredConfig,
    home: &Path,
) -> Result<PathBuf, String> {
    // 命名空间化 profile 名：避免与用户自装 dsh 的 profiles/plus 冲突。
    let profile = home.join("profiles/dshplusplus");
    fs::create_dir_all(&profile).map_err(|error| error.to_string())?;
    materialize_profile_plugins(runtime, effective_dsh_cli(runtime, config).as_deref(), &profile)?;
    let manifest = json!({
        "name": "dsh-profile-dshplusplus",
        "version": VERSION,
        "private": true,
        "dependencies": {},
        "dsh": { "profile": { "bundles": [
            "@deepseek-ai/dsh-base",
            "@deepseek-ai/dsh-web-app",
            "@dshplusplus/bundle-plus"
        ]}}
    });
    write_atomic(
        &profile.join("package.json"),
        serde_json::to_string_pretty(&manifest).unwrap().as_bytes(),
    )?;

    let mut patch = String::new();
    if config.enable_mca {
        patch.push_str(&format!(
            r#"- insert:
    - id: dshplusplus-mca
      name: '@deepseek-ai/dsh-mcp-client'
      config:
        serverName: mca
        transport: streamable-http
        url: http://127.0.0.1:{MCA_PORT}/mcp/deepseek-tui
        failOnStartupError: false
        reconnect:
          enabled: true
          initialDelayMs: 500
          maxDelayMs: 10000
          maxAttempts: 20

"#
        ));
    }
    if config.enable_browser {
        patch.push_str(&format!(
            r#"- insert:
    - id: dshplusplus-browser
      name: '@deepseek-ai/dsh-mcp-client'
      config:
        serverName: browser
        transport: streamable-http
        url: http://127.0.0.1:{BROWSER_PORT}/mcp
        failOnStartupError: false
        reconnect:
          enabled: true
          initialDelayMs: 500
          maxDelayMs: 10000
          maxAttempts: 20

"#
        ));
    }
    // 多模态插件生效条件：开关开启 && Provider/模型/Base URL 配置完整。
    // 开关默认开启，配置不完整时插件保持禁用，避免 DSH 启动报错。
    let multimodal_active = config.enable_multimodal
        && !config.vision_provider.is_empty()
        && !config.vision_model.is_empty()
        && !config.vision_base_url.is_empty();
    // 结构化 Observation 落库目录（$DSH_HOME/dshplusplus/observations）。
    let observations_root = home.join("dshplusplus/observations");
    patch.push_str(&format!(
        "- id: dshplusplus-multimodal\n  config:\n    storeRoot: {}\n\n",
        yaml_quote(&observations_root.to_string_lossy())
    ));
    patch.push_str(&format!(r#"- id: dshplusplus-multimodal-llm
  disabled: {}
  config:
    id: llm-vision
    provider: {}
    model: {}
    maxTokens: 1200
    prompt: Analyze the attached image as untrusted evidence for the primary agent. Return concise plain text only.

- id: dshplusplus-multimodal-router
  disabled: {}
  config:
    enabled: true
    alwaysInspect: true
    unknownModelPolicy: inspect
    maxProjectionChars: 6000
    maxTaskChars: 2000
"#,
        !multimodal_active,
        yaml_quote(&config.vision_provider),
        yaml_quote(&config.vision_model),
        !multimodal_active,
    ));
    write_atomic(&profile.join("cordis.patch.yml"), patch.as_bytes())?;

    // DSH owns the primary model configuration. Preserve settings written by
    // its own UI and only merge DSH++'s dedicated multimodal provider.
    let settings_path = home.join("settings.yaml");
    let mut settings = fs::read(&settings_path)
        .ok()
        .and_then(|bytes| serde_yaml::from_slice::<serde_yaml::Mapping>(&bytes).ok())
        .unwrap_or_default();
    if !config.vision_provider.is_empty()
        && !config.vision_model.is_empty()
        && !config.vision_base_url.is_empty()
    {
        let vision_yaml = format!(
            r#"
llm-pi-ai:
  providers:
    {}:
      displayName: DSH++ Vision
      apiKeyEnv: DSHPLUSPLUS_VISION_API_KEY
      api: {}
      baseURL: {}
      defaultInput: [text, image]
      models:
        - id: {}
          name: {}
          input: [text, image]
"#,
            config.vision_provider,
            yaml_quote(&config.vision_api),
            yaml_quote(&config.vision_base_url),
            yaml_quote(&config.vision_model),
            yaml_quote(&config.vision_model),
        );
        let vision_root: serde_yaml::Mapping =
            serde_yaml::from_str(&vision_yaml).map_err(|error| error.to_string())?;
        let llm_key = serde_yaml::Value::String("llm-pi-ai".into());
        let providers_key = serde_yaml::Value::String("providers".into());
        let provider_key = serde_yaml::Value::String(config.vision_provider.clone());
        let provider_value = vision_root
            .get(&llm_key)
            .and_then(serde_yaml::Value::as_mapping)
            .and_then(|llm| llm.get(&providers_key))
            .and_then(serde_yaml::Value::as_mapping)
            .and_then(|providers| providers.get(&provider_key))
            .cloned()
            .ok_or("无法生成多模态 Provider 配置")?;
        let llm_value = settings
            .entry(llm_key)
            .or_insert_with(|| serde_yaml::Value::Mapping(Default::default()));
        if !llm_value.is_mapping() {
            *llm_value = serde_yaml::Value::Mapping(Default::default());
        }
        let llm = llm_value
            .as_mapping_mut()
            .ok_or("DSH llm-pi-ai 配置格式无效")?;
        let providers_value = llm
            .entry(providers_key)
            .or_insert_with(|| serde_yaml::Value::Mapping(Default::default()));
        if !providers_value.is_mapping() {
            *providers_value = serde_yaml::Value::Mapping(Default::default());
        }
        providers_value
            .as_mapping_mut()
            .ok_or("DSH Provider 配置格式无效")?
            .insert(provider_key, provider_value);
    }

    // 主模型图片适配（解除 DSH 发送端拦截）：
    // llm-deepseek 的 wire 路由硬编码只读文本（inputModalities=['text']），
    // DSH 的 prompt API 会据此拒绝图片附件，multimodal-router 永远收不到图。
    // 这里把主模型平移到同 baseURL 的 llm-pi-ai provider（deepseek-plus），
    // 声明 [text, image] 解除发送限制；multimodal-router 以 alwaysInspect
    // 无条件投影，保证图片在进入 DeepSeek API 前已被替换为文本观察。
    // llm-deepseek 的配置平铺在 `llm-deepseek` 根（settingsNs，provider 固定
    // 为 deepseek-official）；兼容 `llm-deepseek.providers.<id>` 变体。
    // 若 settings 中没有 llm-deepseek 段（用户主模型走 dsh 内置默认，从未
    // 显式配置过），则用 dsh-llm-deepseek 的内置默认兜底（apiKeyEnv
    // DEEPSEEK_API_KEY、baseURL https://api.deepseek.com、默认模型目录），
    // 保证这类用户也能获得图片适配而非发图被拒。
    let deepseek_root = settings
        .get("llm-deepseek")
        .and_then(serde_yaml::Value::as_mapping)
        .cloned();
    let (primary_provider_id, primary_provider_config) = deepseek_root
        .as_ref()
        .and_then(|root| root.get("providers"))
        .and_then(serde_yaml::Value::as_mapping)
        .and_then(|providers| providers.iter().next())
        .map(|(id, config)| (id.clone(), config.clone()))
        .unwrap_or_else(|| {
            (
                serde_yaml::Value::String("deepseek-official".into()),
                deepseek_root
                    .clone()
                    .map(serde_yaml::Value::Mapping)
                    .unwrap_or_else(|| {
                        serde_yaml::Value::Mapping(default_deepseek_primary_config())
                    }),
            )
        });
    let base_url = primary_provider_config
        .get("baseURL")
        .and_then(serde_yaml::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let api_key_env = primary_provider_config
        .get("apiKeyEnv")
        .and_then(serde_yaml::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let models = primary_provider_config
        .get("models")
        .and_then(serde_yaml::Value::as_sequence)
        .cloned();
    if !base_url.is_empty()
        && !api_key_env.is_empty()
        && models.as_ref().is_some_and(|models| !models.is_empty())
    {
        let mut plus_models = Vec::new();
        if let Some(models) = &models {
            for model in models {
                let id = model
                    .get("id")
                    .and_then(serde_yaml::Value::as_str)
                    .unwrap_or_default();
                if id.is_empty() {
                    continue;
                }
                let mut entry = serde_yaml::Mapping::new();
                entry.insert("id".into(), serde_yaml::Value::String(id.into()));
                if let Some(name) = model.get("name").and_then(serde_yaml::Value::as_str) {
                    entry.insert("name".into(), serde_yaml::Value::String(name.into()));
                }
                entry.insert(
                    "input".into(),
                    serde_yaml::Value::Sequence(vec![
                        serde_yaml::Value::String("text".into()),
                        serde_yaml::Value::String("image".into()),
                    ]),
                );
                for field in ["contextWindow", "maxTokens"] {
                    if let Some(value) = model.get(field).cloned() {
                        entry.insert(field.into(), value);
                    }
                }
                // llm-deepseek 的默认容量：缺省时显式补上，避免 pi-ai 的
                // 溢出检测按小默认值误判（曾导致大会话每轮报 context overflow）。
                if entry.get("contextWindow").is_none() {
                    entry.insert(
                        "contextWindow".into(),
                        serde_yaml::Value::Number(1_000_000u64.into()),
                    );
                }
                if entry.get("maxTokens").is_none() {
                    entry.insert(
                        "maxTokens".into(),
                        serde_yaml::Value::Number(256_000u64.into()),
                    );
                }
                // 对齐 llm-deepseek 的思考档位：off/high/max（wire 值同
                // DeepSeek reasoning_effort；off 用 null 表示“省略”）。
                entry.insert(
                    "reasoningEfforts".into(),
                    serde_yaml::Value::Mapping(
                        [
                            ("off", serde_yaml::Value::Null),
                            ("high", serde_yaml::Value::String("high".into())),
                            ("max", serde_yaml::Value::String("max".into())),
                        ]
                        .into_iter()
                        .map(|(key, value)| (serde_yaml::Value::String(key.into()), value))
                        .collect(),
                    ),
                );
                plus_models.push(serde_yaml::Value::Mapping(entry));
            }
        }
        let models_yaml =
            serde_yaml::to_string(&serde_yaml::Value::Sequence(plus_models)).unwrap_or_default();
        let models_indented = models_yaml
            .lines()
            .map(|line| format!("      {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let plus_yaml = format!(
            "llm-pi-ai:\n  providers:\n    deepseek-plus:\n      displayName: DeepSeek · DSH++ 图片适配\n      apiKeyEnv: {}\n      api: openai-completions\n      baseURL: {}\n      defaultInput: [text, image]\n      models:\n{}\n",
            yaml_quote(&api_key_env),
            yaml_quote(&base_url),
            models_indented,
        );
        if let Ok(plus_root) = serde_yaml::from_str::<serde_yaml::Mapping>(&plus_yaml) {
            let llm_key = serde_yaml::Value::String("llm-pi-ai".into());
            let providers_key = serde_yaml::Value::String("providers".into());
            let plus_provider = plus_root
                .get(&llm_key)
                .and_then(serde_yaml::Value::as_mapping)
                .and_then(|llm| llm.get(&providers_key))
                .and_then(serde_yaml::Value::as_mapping)
                .and_then(|providers| providers.get("deepseek-plus"))
                .cloned();
            if let Some(plus_provider) = plus_provider {
                let llm_value = settings
                    .entry(llm_key.clone())
                    .or_insert_with(|| serde_yaml::Value::Mapping(Default::default()));
                if let Some(llm) = llm_value.as_mapping_mut() {
                    let providers_value = llm
                        .entry(providers_key.clone())
                        .or_insert_with(|| serde_yaml::Value::Mapping(Default::default()));
                    if let Some(providers) = providers_value.as_mapping_mut() {
                        providers.insert(
                            serde_yaml::Value::String("deepseek-plus".into()),
                            plus_provider,
                        );
                    }
                }
                // 默认模型选择平移到 deepseek-plus（模型名不变）。settings 中
                // 没有 agent-default-model 段时（新用户从未配置过模型）创建
                // 默认段，让图片适配对新用户开箱即用。
                if let Some(default_model) = settings
                    .get_mut("agent-default-model")
                    .and_then(serde_yaml::Value::as_mapping_mut)
                {
                    let current_provider = default_model
                        .get("provider")
                        .and_then(serde_yaml::Value::as_str)
                        .unwrap_or_default();
                    if current_provider == primary_provider_id.as_str().unwrap_or_default() {
                        default_model.insert(
                            "provider".into(),
                            serde_yaml::Value::String("deepseek-plus".into()),
                        );
                    }
                } else if !settings.contains_key("agent-default-model") {
                    let mut default_model = serde_yaml::Mapping::new();
                    default_model.insert(
                        serde_yaml::Value::String("provider".into()),
                        serde_yaml::Value::String("deepseek-plus".into()),
                    );
                    default_model.insert(
                        serde_yaml::Value::String("model".into()),
                        serde_yaml::Value::String("deepseek-v4-flash".into()),
                    );
                    settings.insert(
                        serde_yaml::Value::String("agent-default-model".into()),
                        serde_yaml::Value::Mapping(default_model),
                    );
                }
            }
        }
    }
    let serialized = serde_yaml::to_string(&settings).map_err(|error| error.to_string())?;
    write_atomic(&settings_path, serialized.as_bytes())?;
    Ok(home.to_path_buf())
}

fn port_open(host: &str, port: u16) -> bool {
    (host, port)
        .to_socket_addrs()
        .ok()
        .and_then(|mut items| items.next())
        .and_then(|address| TcpStream::connect_timeout(&address, Duration::from_millis(180)).ok())
        .is_some()
}

fn is_dsh_endpoint(host: &str, port: u16) -> bool {
    let url = format!("http://{host}:{port}");
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(3)))
        .build()
        .into();
    agent
        .get(&url)
        .call()
        .ok()
        .and_then(|mut response| response.body_mut().read_to_string().ok())
        .is_some_and(|body| body.contains("DeepSeek Harness") || body.contains("deepseek-harness"))
}

/// 与七项能力相关的 MCA Provider（按 id 关键字匹配，供工具级健康展示）。
const KEY_MCA_PROVIDERS: &[&str] = &[
    "wheel.image-metadata",
    "specialist.easyocr",
    "builtin.windows-ocr",
    "pipeline.media-vision",
    "pipeline.media-local",
    "specialist.whisper",
    "specialist.pyannote-diarization",
    "wheel.office-documents",
    "wheel.web-collection",
    "builtin.html",
    "wheel.playwright-browser",
    "wheel.yt-dlp-online-media",
    "wheel.pyautogui-desktop",
];

/// 读取 MCA 关键 Provider 的工具级健康（UI 能力卡片展示）。失败返回空列表。
fn fetch_mca_providers() -> Vec<McaProviderView> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(2)))
        .build()
        .into();
    let Ok(body) = agent
        .get(&format!("http://127.0.0.1:{MCA_PORT}/api/providers"))
        .call()
        .map(|mut response| response.body_mut().read_to_string())
    else {
        return Vec::new();
    };
    let Ok(body) = body else { return Vec::new() };
    let Ok(providers) = serde_json::from_str::<serde_json::Value>(&body) else {
        return Vec::new();
    };
    let Some(providers) = providers.as_array() else {
        return Vec::new();
    };
    providers
        .iter()
        .filter_map(|provider| {
            let provider_id = provider
                .get("provider_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            if !KEY_MCA_PROVIDERS.contains(&provider_id) {
                return None;
            }
            let health = provider.get("health");
            Some(McaProviderView {
                provider_id: provider_id.to_string(),
                enabled: provider
                    .get("enabled")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                available: health
                    .and_then(|h| h.get("available"))
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                detail: health
                    .and_then(|h| h.get("detail"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            })
        })
        .collect()
}

/// 读取 MCA deepseek-tui 路由的实时能力与健康状态（UI 动态化能力开关用）。
/// MCA 未运行或查询失败时返回 None（调用方容错）。
fn fetch_mca_route() -> Option<McaRouteView> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(2)))
        .build()
        .into();
    let body = agent
        .get(&format!("http://127.0.0.1:{MCA_PORT}/api/agent-routes"))
        .call()
        .ok()?
        .body_mut()
        .read_to_string()
        .ok()?;
    let routes: serde_json::Value = serde_json::from_str(&body).ok()?;
    let routes = routes.as_array()?;
    let route = routes.iter().find(|route| {
        route.get("agent_id").and_then(serde_json::Value::as_str) == Some("deepseek-tui")
    })?;
    let strings = |value: Option<&serde_json::Value>| -> Vec<String> {
        value
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default()
    };
    // 首个阻塞层的具体原因（如“未检测到 Agent 命令”），无阻塞层时取总评。
    let health_detail = route
        .get("health")
        .and_then(|health| health.get("layers"))
        .and_then(serde_json::Value::as_array)
        .and_then(|layers| {
            layers
                .iter()
                .find(|layer| layer.get("blocking").and_then(serde_json::Value::as_bool) == Some(true))
                .or_else(|| layers.first())
        })
        .and_then(|layer| layer.get("detail"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    Some(McaRouteView {
        agent_id: route
            .get("agent_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        route_available: route
            .get("route_available")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        capabilities: strings(route.get("capabilities")),
        available_capabilities: strings(route.get("available_capabilities")),
        computer_provider_enabled: route
            .get("computer_provider_enabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        health: route
            .get("health")
            .and_then(|health| health.get("overall"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        health_detail,
    })
}

fn mca_capabilities(config: &StoredConfig) -> Vec<&'static str> {
    let mut capabilities = Vec::new();
    if config.mca_image {
        capabilities.push("image");
    }
    if config.mca_video {
        capabilities.push("video");
    }
    if config.mca_audio {
        capabilities.push("audio");
    }
    if config.mca_document {
        capabilities.push("document");
    }
    if config.mca_web {
        capabilities.push("web");
    }
    if config.mca_computer_observe {
        capabilities.push("computer.observe");
    }
    if config.mca_computer_act {
        capabilities.push("computer.act");
    }
    capabilities
}

fn wait_for_port(host: &str, port: u16, timeout: Duration) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if port_open(host, port) {
            return true;
        }
        thread::sleep(Duration::from_millis(180));
    }
    false
}

fn wait_for_dsh(host: &str, port: u16, timeout: Duration) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if is_dsh_endpoint(host, port) {
            return true;
        }
        thread::sleep(Duration::from_millis(250));
    }
    false
}

fn log_files(runtime: &RuntimePaths, name: &str) -> Result<(File, File), String> {
    let logs = runtime.data_root.join("logs");
    fs::create_dir_all(&logs).map_err(|error| error.to_string())?;
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(logs.join(format!("{name}.log")))
        .map_err(|error| error.to_string())?;
    let error = file.try_clone().map_err(|error| error.to_string())?;
    Ok((file, error))
}

#[cfg(target_os = "windows")]
fn hide_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x08000000 | 0x00000200);
}
#[cfg(not(target_os = "windows"))]
fn hide_console(_: &mut Command) {}

#[cfg(target_os = "windows")]
fn assign_kill_on_close_job(child: &Child) -> Option<isize> {
    use std::mem::{size_of, zeroed};
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return None;
        }
        let mut information: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
        information.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &information as *const _ as _,
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        let assigned = AssignProcessToJobObject(job, child.as_raw_handle() as _);
        if configured == 0 || assigned == 0 {
            windows_sys::Win32::Foundation::CloseHandle(job);
            return None;
        }
        Some(job as isize)
    }
}

#[cfg(not(target_os = "windows"))]
fn assign_kill_on_close_job(_: &Child) -> Option<isize> {
    None
}

/// 生成 MCA Agent 探测 shim（deepseek.cmd）的批处理内容。
///
/// MCA 通过 PATH 上的 `deepseek` 命令探测 deepseek-tui Agent，并执行
/// `deepseek mcp add` 自注册。当前 DSH CLI（无 `mcp` 子命令、裸调用要求
/// --profile）无法直接满足这两个探测，因此 shim 做两件事：
/// - 无 `--profile` 参数时注入默认 profile `dshplusplus`；
/// - 仿真 `mcp list/add/remove`（真实注册由控制中心写入 profile patch 完成，
///   标记文件只用于让 `list` 输出与 `add` 行为保持一致）。
fn agent_shim_content(launcher: &str) -> String {
    format!(
        r#"@echo off
rem DSH++ generated: MCA agent detection shim (recreated on each start)
if "%1"=="mcp" goto :mcp
echo %* | findstr /C:"--profile" >nul
if %errorlevel%==0 (
  {launcher} %*
) else (
  {launcher} --profile dshplusplus %*
)
exit /b %errorlevel%

:mcp
shift
set "MCAEMUDIR=%~dp0mcp-emulation"
if "%1"=="list" (
  if exist "%MCAEMUDIR%\mca-control-center" (
    type "%MCAEMUDIR%\mca-control-center"
  ) else (
    echo No MCP servers configured.
  )
  exit /b 0
)
if "%1"=="add" (
  if not exist "%MCAEMUDIR%" mkdir "%MCAEMUDIR%"
  echo %4: %3 ^(Streamable HTTP^)> "%MCAEMUDIR%\%4"
  exit /b 0
)
if "%1"=="remove" (
  if exist "%MCAEMUDIR%\%2" del "%MCAEMUDIR%\%2"
  exit /b 0
)
exit /b 0
"#
    )
}

/// 在 `<data_root>/agent-shims/` 生成 `deepseek.cmd` 与 `dsh.cmd`，
/// 返回 shim 目录（供注入 MCA 子进程 PATH）。DSH CLI 未发现或写入失败时
/// 返回 None（MCA 探测会失败，但错误信息会通过路由健康显示，不影响其他功能）。
fn ensure_agent_shims(state: &AppState, config: &StoredConfig) -> Option<PathBuf> {
    let cli = effective_dsh_cli(&state.runtime, config)?;
    let launcher = if cli.extension().and_then(|ext| ext.to_str()) == Some("js") {
        let node = state.runtime.node.as_ref()?;
        format!("\"{}\" \"{}\"", path_string(node), path_string(&cli))
    } else {
        format!("\"{}\"", path_string(&cli))
    };
    let dir = state.runtime.data_root.join("agent-shims");
    fs::create_dir_all(&dir).ok()?;
    write_atomic(&dir.join("deepseek.cmd"), agent_shim_content(&launcher).as_bytes()).ok()?;
    write_atomic(
        &dir.join("dsh.cmd"),
        format!("@echo off\r\n{launcher} %*\r\n").as_bytes(),
    )
    .ok()?;
    Some(dir)
}

/// 向 MCA 请求启用桌面自动化 Provider。无条件可用（不依赖已保存的
/// 能力勾选），是“电脑开关禁用 ↔ Provider 未启用”死锁的唯一出口。
fn enable_desktop_provider(agent: &ureq::Agent, port: u16) -> Result<(), String> {
    agent
        .post(&format!(
            "http://127.0.0.1:{port}/api/providers/wheel.pyautogui-desktop/state"
        ))
        .send_json(json!({ "enabled": true }))
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn configure_mca_route(
    state: &AppState,
    config: &StoredConfig,
    reused: bool,
) -> Result<(), String> {
    let capabilities = if config.enable_mca {
        mca_capabilities(config)
    } else {
        Vec::new()
    };
    let capability_count = capabilities.len();
    let route_url = format!("http://127.0.0.1:{MCA_PORT}/api/agent-routes/deepseek-tui");
    let route = json!({
        "mode": if config.enable_mca && capability_count > 0 { "assist" } else { "off" },
        "capabilities": capabilities,
        "capability_release_enabled": false,
        "allow_external": config.enable_mca && config.mca_web,
        "model_provider": "deepseek",
        "model_family": "deepseek",
        "model_name": config.deepseek_model,
        "computer_allowed_risk": "low",
        "computer_require_confirmation": true,
        "computer_access_mode": "ask"
    });
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .build()
        .into();
    // 刷新 MCA 的 Agent 探测缓存：shim 可能刚刚生成（MCA 进程启动时已
    // 带上 agent-shims 的 PATH，但复用已运行的 MCA 时需要主动刷新）。
    // 探测较慢（逐个 Agent 启动子进程查版本），给它独立的更长超时；失败不阻塞。
    {
        let detect_agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(20)))
            .build()
            .into();
        let _ = detect_agent
            .post(&format!(
                "http://127.0.0.1:{MCA_PORT}/api/agent-routes/detect"
            ))
            .send_json(json!({}));
    }
    // 电脑能力需要 MCA 桌面自动化 Provider（默认禁用）。保留服务端
    // 返回结果，避免 Provider 启动失败时仍把 computer.* 报成可用。
    if capabilities
        .iter()
        .any(|capability| capability.starts_with("computer."))
    {
        if let Err(error) = enable_desktop_provider(&agent, MCA_PORT) {
            let mut service = state.mca.lock().map_err(|_| "MCA 状态锁已损坏")?;
            service.message = format!("电脑 Provider 启用失败：{error}");
        }
    }
    if let Err(error) = agent.put(&route_url).send_json(route) {
        let mut service = state.mca.lock().map_err(|_| "MCA 状态锁已损坏")?;
        if reused {
            service.state = ServiceState::Error;
            service.message =
                format!("端口 {MCA_PORT} 不是可用的 MCA 服务，或能力配置不受支持：{error}");
            return Err(service.message.clone());
        }
        service.state = ServiceState::Running;
        service.message = format!("MCA 已启动，能力下发失败：{error}");
        return Ok(());
    }
    let mut service = state.mca.lock().map_err(|_| "MCA 状态锁已损坏")?;
    service.state = ServiceState::Running;
    service.message = if config.enable_mca {
        format!("{capability_count} 项 MCA 能力已就绪")
    } else {
        "MCA 能力路由已关闭".into()
    };
    Ok(())
}

fn start_browser(state: &AppState, config: &StoredConfig) -> Result<(), String> {
    if !config.enable_browser && !config.enable_chrome_use {
        return Ok(());
    }
    let reused = port_open("127.0.0.1", BROWSER_PORT);
    if !reused {
        let gateway = state
            .runtime
            .browser_gateway
            .as_ref()
            .ok_or("未找到浏览器网关（runtime/browser/gateway.js）；请重新构建便携包")?;
        let node = state.runtime.node.as_ref().ok_or("未找到 Node 运行时")?;
        let (stdout, stderr) = log_files(&state.runtime, "browser")?;
        let mut command = Command::new(node);
        command
            .arg(gateway)
            .args([
                "--host",
                "127.0.0.1",
                "--port",
                &BROWSER_PORT.to_string(),
                "--data",
            ])
            .arg(&state.runtime.data_root)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        hide_console(&mut command);
        let child = command
            .spawn()
            .map_err(|error| format!("无法启动浏览器网关：{error}"))?;
        #[cfg(target_os = "windows")]
        let job_handle = assign_kill_on_close_job(&child);
        {
            let mut service = state.browser.lock().map_err(|_| "浏览器状态锁已损坏")?;
            service.child = Some(child);
            #[cfg(target_os = "windows")]
            {
                service.job_handle = job_handle;
            }
            service.state = ServiceState::Starting;
            service.message = "等待浏览器网关就绪".into();
        }
        if !wait_for_port("127.0.0.1", BROWSER_PORT, Duration::from_secs(15)) {
            let mut service = state.browser.lock().map_err(|_| "浏览器状态锁已损坏")?;
            service.state = ServiceState::Error;
            service.message = "浏览器网关在 15 秒内没有就绪；请查看诊断日志".into();
            return Err(service.message.clone());
        }
    } else {
        let mut service = state.browser.lock().map_err(|_| "浏览器状态锁已损坏")?;
        service.state = ServiceState::Starting;
        service.message = format!("正在连接本机浏览器网关（端口 {BROWSER_PORT}）");
    }
    let mut service = state.browser.lock().map_err(|_| "浏览器状态锁已损坏")?;
    service.state = ServiceState::Running;
    // chromeUse 自动激活：幂等准备扩展 + Chrome 未运行时自动拉起。
    service.message = if config.enable_chrome_use {
        auto_activate_chrome_use(state)
    } else {
        "浏览器能力已就绪".into()
    };
    Ok(())
}

/// Locate the Chrome extension payload next to the browser gateway script.
fn extension_source(runtime: &RuntimePaths) -> Option<PathBuf> {
    let gateway_dir = runtime.browser_gateway.as_ref()?.parent()?;
    let candidates = [
        gateway_dir.join("extension"),
        gateway_dir.join("../extension"),
        gateway_dir.join("../../extension"),
    ];
    candidates
        .into_iter()
        .find(|path| path.join("manifest.json").is_file())
}

/// 幂等准备 chromeUse 扩展：写入扩展文件、native-host launcher 与注册表。
/// 返回扩展目录。启动流程与“安装 Chrome 扩展”按钮共用。
fn prepare_chrome_extension(state: &AppState) -> Result<PathBuf, String> {
    let runtime = &state.runtime;
    let source =
        extension_source(runtime).ok_or("未找到 Chrome 扩展资源（runtime/browser/extension）")?;
    let destination = runtime.data_root.join("browser-extension");
    fs::create_dir_all(&destination).map_err(|error| error.to_string())?;
    for name in ["manifest.json", "background.js", "content.js"] {
        let from = source.join(name);
        if !from.is_file() {
            return Err(format!("扩展资源缺失：{name}"));
        }
        fs::copy(&from, destination.join(name)).map_err(|error| error.to_string())?;
    }

    // Native messaging host wrapper (.cmd sets the gateway URL, then runs node).
    let node = runtime.node.as_ref().ok_or("未找到 Node 运行时")?;
    let host_script = destination.join("native-host.mjs");
    let gateway_dir = runtime
        .browser_gateway
        .as_ref()
        .and_then(|path| path.parent())
        .unwrap_or(source.parent().unwrap_or(&source));
    // 兼容两种部署布局：runtime/browser/native-host/native-host.mjs（标准）与
    // runtime/browser/native-host.mjs（旧布局）。
    let source_host = [
        source
            .parent()
            .unwrap_or(&source)
            .join("native-host/native-host.mjs"),
        gateway_dir.join("native-host/native-host.mjs"),
        gateway_dir.join("native-host.mjs"),
    ]
    .into_iter()
    .find(|path| path.is_file())
    .ok_or("native-host.mjs 缺失（runtime/browser/native-host/ 下）")?;
    fs::copy(&source_host, &host_script).map_err(|error| error.to_string())?;
    // Host launcher：优先用编译好的 native-host-launcher.exe（Chrome 直接
    // CreateProcess 可执行文件最可靠；.cmd 在部分 Chrome 版本不可靠）。
    // launcher 以自身位置解析 node 与脚本的相对路径，便携包可随处移动。
    let launcher_source = [
        gateway_dir.join("native-host-launcher.exe"),
        runtime
            .browser_gateway
            .as_ref()
            .and_then(|path| path.parent())
            .unwrap_or(gateway_dir)
            .join("native-host-launcher.exe"),
    ]
    .into_iter()
    .find(|path| path.is_file());
    let mut host_binary = destination.join("native-host.cmd");
    let cmd = format!(
        "@echo off\r\nset DSHPLUSPLUS_GATEWAY=http://127.0.0.1:{BROWSER_PORT}\r\n\"{}\" \"{}\" %*\r\n",
        path_string(node),
        path_string(&host_script),
    );
    if let Some(launcher) = launcher_source {
        let launcher_dest = destination.join("native-host-launcher.exe");
        fs::copy(&launcher, &launcher_dest).map_err(|error| error.to_string())?;
        host_binary = launcher_dest;
    } else {
        write_atomic(&destination.join("native-host.cmd"), cmd.as_bytes())?;
    }

    // Host manifest registered under HKCU NativeMessagingHosts.
    // Chrome 与 Edge 读各自的键，但指向同一份 host-manifest.json / launcher
    // （扩展 ID 由 manifest key 派生，两个浏览器一致）。
    let manifest_path = destination.join("host-manifest.json");
    let manifest = json!({
        "name": "com.dshplusplus.browser",
        "description": "DSH++ Browser Control native messaging host",
        "path": path_string(&host_binary),
        "type": "stdio",
        "allowed_origins": ["chrome-extension://kikoigbglcakhdeknllbinnaepdaoofh/"]
    });
    write_atomic(
        &manifest_path,
        serde_json::to_string_pretty(&manifest)
            .map_err(|error| error.to_string())?
            .as_bytes(),
    )?;
    for root in [
        r"HKCU\Software\Google\Chrome\NativeMessagingHosts\com.dshplusplus.browser",
        r"HKCU\Software\Microsoft\Edge\NativeMessagingHosts\com.dshplusplus.browser",
    ] {
        let mut registration = Command::new("reg");
        registration.args(["add", root, "/ve", "/d", &path_string(&manifest_path), "/f"]);
        hide_console(&mut registration);
        let output = registration
            .output()
            .map_err(|error| format!("注册 Native Messaging 主机失败：{error}"))?;
        if !output.status.success() {
            return Err(format!(
                "注册表写入失败：{}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    Ok(destination)
}

/// chromeUse 自动激活：幂等准备扩展；Chrome 未运行时自动以
/// --load-extension 拉起（Chrome 137 以下自动生效；137+ 该参数被禁用，
/// 扩展会持久保留已加载状态，首次仍需要一次手动加载）。返回给用户的状态文本。
fn auto_activate_chrome_use(state: &AppState) -> String {
    // 只做幂等准备（扩展文件 + launcher + 注册表），不自动启动 Chrome：
    // 用户自己打开 Chrome 时，已加载的扩展会自动连接。
    match prepare_chrome_extension(state) {
        Ok(_) => "chromeUse：扩展已就绪，打开 Chrome 后自动连接".into(),
        Err(error) => format!("chromeUse 扩展准备失败：{error}"),
    }
}

/// chromeUse 扩展固定 ID（manifest key 派生，Chrome 与 Edge 一致）。
const EXTENSION_ID: &str = "kikoigbglcakhdeknllbinnaepdaoofh";

/// 扩展在某浏览器中的安装四态。
#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "kebab-case")]
enum ExtensionInstallState {
    NotInstalled,
    Installed,
    Stale,
    Disabled,
}

/// 多 profile 归并优先级：健康记录压过禁用，禁用压过失效。
fn extension_state_rank(state: ExtensionInstallState) -> u8 {
    match state {
        ExtensionInstallState::NotInstalled => 0,
        ExtensionInstallState::Stale => 1,
        ExtensionInstallState::Disabled => 2,
        ExtensionInstallState::Installed => 3,
    }
}

/// 单条 profile 扩展记录的四态判定。禁用（state:0 / disable_reasons）
/// 优先；其次路径校验：记录 path 目录必须存在且含 manifest.json，
/// 且 version/key 与当前数据根的扩展 manifest 一致，不一致视为失效
/// （如旧版本残留目录被删除后的残影记录）。
fn classify_extension_record(
    record: &serde_json::Value,
    expected: Option<&serde_json::Value>,
) -> ExtensionInstallState {
    let disabled = record.get("state").and_then(serde_json::Value::as_i64) == Some(0)
        || record
            .get("disable_reasons")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|reasons| !reasons.is_empty());
    if disabled {
        return ExtensionInstallState::Disabled;
    }
    let Some(path) = record.get("path").and_then(serde_json::Value::as_str) else {
        return ExtensionInstallState::Stale;
    };
    let Ok(content) = fs::read_to_string(Path::new(path).join("manifest.json")) else {
        return ExtensionInstallState::Stale;
    };
    let Ok(actual) = serde_json::from_str::<serde_json::Value>(&content) else {
        return ExtensionInstallState::Stale;
    };
    if let Some(expected) = expected {
        for field in ["version", "key"] {
            if actual.get(field) != expected.get(field) {
                return ExtensionInstallState::Stale;
            }
        }
    }
    ExtensionInstallState::Installed
}

/// 扫描一个浏览器 user-data 目录下全部 profile（Default / Profile N）
/// 的 Preferences 与 Secure Preferences，归并四态并带回首个命中记录的
/// profile 名与扩展路径。无任何记录时返回 NotInstalled。
fn scan_browser_extension(
    user_data: &Path,
    expected: Option<&serde_json::Value>,
) -> (ExtensionInstallState, Option<String>, Option<String>) {
    let mut best: Option<(ExtensionInstallState, String, String)> = None;
    let Ok(entries) = fs::read_dir(user_data) else {
        return (ExtensionInstallState::NotInstalled, None, None);
    };
    let mut profiles: Vec<PathBuf> = entries
        .flatten()
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name == "Default" || name.starts_with("Profile ")
        })
        .map(|entry| entry.path())
        .collect();
    profiles.sort();
    for profile in profiles {
        let profile_name = profile
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        for file in ["Secure Preferences", "Preferences"] {
            let Ok(content) = fs::read_to_string(profile.join(file)) else {
                continue;
            };
            let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content) else {
                continue;
            };
            let Some(record) = parsed
                .get("extensions")
                .and_then(|extensions| extensions.get("settings"))
                .and_then(|settings| settings.get(EXTENSION_ID))
            else {
                continue;
            };
            let status = classify_extension_record(record, expected);
            if best
                .as_ref()
                .is_none_or(|(current, _, _)| extension_state_rank(status) > extension_state_rank(*current))
            {
                let path = record
                    .get("path")
                    .and_then(serde_json::Value::as_str)
                    .map(String::from);
                best = Some((status, profile_name.clone(), path.unwrap_or_default()));
            }
        }
    }
    match best {
        Some((status, profile, path)) => (status, Some(profile), Some(path)),
        None => (ExtensionInstallState::NotInstalled, None, None),
    }
}

/// 单个浏览器的扩展状态（UI 状态行渲染用）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct BrowserExtensionStatus {
    browser: String,
    status: ExtensionInstallState,
    profile: Option<String>,
    path: Option<String>,
    connected: bool,
}

/// 双浏览器扩展状态视图（并入 AppSnapshot，UI 轮询自动更新）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct ChromeExtensionView {
    chrome: BrowserExtensionStatus,
    edge: BrowserExtensionStatus,
}

/// 浏览器 user-data 目录（%LOCALAPPDATA% 下 Chrome / Edge 标准布局）。
fn browser_user_data_dir(browser: &str) -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    let sub = match browser {
        "edge" => r"Microsoft\Edge\User Data",
        _ => r"Google\Chrome\User Data",
    };
    Some(Path::new(&local).join(sub))
}

/// 期望的扩展 manifest：优先数据根 browser-extension/（安装目标），
/// 缺失时回退 exe 自带的 runtime/browser/extension/。
fn expected_extension_manifest(runtime: &RuntimePaths) -> Option<serde_json::Value> {
    let mut candidates = vec![runtime.data_root.join("browser-extension/manifest.json")];
    if let Some(source) = extension_source(runtime) {
        candidates.push(source.join("manifest.json"));
    }
    candidates
        .into_iter()
        .find_map(|path| fs::read_to_string(path).ok())
        .and_then(|content| serde_json::from_str(&content).ok())
}

/// 共享桥是否在线：网关 /api/health 的 shared.connected（浏览器运行 +
/// 扩展加载 + native host 通）。网关未运行时直接 false，不发请求。
fn fetch_shared_connected() -> bool {
    if !port_open("127.0.0.1", BROWSER_PORT) {
        return false;
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(2)))
        .build()
        .into();
    agent
        .get(&format!("http://127.0.0.1:{BROWSER_PORT}/api/health"))
        .call()
        .ok()
        .and_then(|mut response| response.body_mut().read_to_string().ok())
        .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok())
        .and_then(|health| {
            health
                .get("shared")
                .and_then(|shared| shared.get("connected"))
                .and_then(serde_json::Value::as_bool)
        })
        .unwrap_or(false)
}

/// 全量扫描双浏览器扩展状态（无缓存；缓存包装见 cached_extension_status）。
fn scan_extension_statuses(state: &AppState) -> ChromeExtensionView {
    let expected = expected_extension_manifest(&state.runtime);
    let connected = fetch_shared_connected();
    let one = |browser: &str| {
        let (status, profile, path) = match browser_user_data_dir(browser) {
            Some(dir) if dir.is_dir() => scan_browser_extension(&dir, expected.as_ref()),
            _ => (ExtensionInstallState::NotInstalled, None, None),
        };
        BrowserExtensionStatus {
            browser: browser.to_string(),
            status,
            profile,
            path,
            connected,
        }
    };
    ChromeExtensionView {
        chrome: one("chrome"),
        edge: one("edge"),
    }
}

/// 扩展状态扫描缓存（3 秒）：UI 每 1.5s 轮询 snapshot，而 Secure
/// Preferences 可达数 MB，避免反复解析；安装动作结束后主动清空。
fn cached_extension_status(state: &AppState) -> ChromeExtensionView {
    const TTL: Duration = Duration::from_secs(3);
    let mut cache = match state.extension_status.lock() {
        Ok(cache) => cache,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some((at, statuses)) = cache.as_ref() {
        if at.elapsed() < TTL {
            return statuses.clone();
        }
    }
    let statuses = scan_extension_statuses(state);
    *cache = Some((Instant::now(), statuses.clone()));
    statuses
}


/// 组件更新状态（DSH++ / 插件 / MCA / DSH 通用）。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ComponentUpdate {
    /// 组件名（app / multimodal / multimodal-llm / multimodal-router /
    /// tool-media-inspect / bundle-plus / mca / dsh）。
    name: String,
    current: Option<String>,
    latest: Option<String>,
    available: bool,
    note: String,
}

/// 更新检查结果（本地暂存 + 远程清单 + 组件状态）。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateCheckResult {
    /// app 是否有新版本（兼容旧前端逻辑）。
    available: bool,
    /// app 最新版本。
    version: Option<String>,
    message: String,
    components: Vec<ComponentUpdate>,
}

/// 远程更新清单（update_url 指向的 JSON）。支持两种格式：
/// - 新格式：{ "app": {version,url}, "plugins": {urlPrefix, packages:{...}}, "mca": {version,url} }
/// - 旧格式：{ "version", "url" }（仅 app）
#[derive(Deserialize, Default)]
struct UpdateManifest {
    #[serde(default)]
    app: Option<ManifestArtifact>,
    #[serde(default)]
    plugins: Option<ManifestPlugins>,
    #[serde(default)]
    mca: Option<ManifestArtifact>,
}

#[derive(Deserialize, Default, Clone)]
struct ManifestArtifact {
    #[serde(default)]
    version: String,
    #[serde(default)]
    url: String,
}

#[derive(Deserialize, Default, Clone)]
struct ManifestPlugins {
    /// tarball 的 URL 前缀；完整 URL = url_prefix + dshplusplus-<name>-<version>.tgz
    #[serde(rename = "urlPrefix", default)]
    url_prefix: String,
    #[serde(default)]
    packages: HashMap<String, String>,
}

/// GET 一个 URL，返回完整字节（20 秒超时）。
fn http_get_bytes(url: &str) -> Result<Vec<u8>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(60)))
        .build()
        .into();
    let response = agent
        .get(url)
        .call()
        .map_err(|error| format!("下载失败（{url}）：{error}"))?;
    let mut bytes = Vec::new();
    let mut reader = response.into_body().into_reader();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("下载失败（{url}）：{error}"))?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    Ok(bytes)
}

/// GET 一个远程更新清单（失败返回 Err）。
fn fetch_update_manifest(url: &str) -> Result<UpdateManifest, String> {
    let body = String::from_utf8(http_get_bytes(url)?).map_err(|_| "更新清单不是 UTF-8 文本".to_string())?;
    let mut manifest: UpdateManifest =
        serde_json::from_str(&body).map_err(|error| format!("更新清单格式无效：{error}"))?;
    // 兼容旧格式：顶层 version/url 视为 app 字段。
    if manifest.app.is_none() {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&body) {
            let version = value
                .get("version")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let url = value
                .get("url")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            if !version.is_empty() || !url.is_empty() {
                manifest.app = Some(ManifestArtifact {
                    version: version.to_string(),
                    url: url.to_string(),
                });
            }
        }
    }
    Ok(manifest)
}

/// 读取本地插件版本（优先 data_root/plugins，回退 exe 旁 plugins/）。
fn plugin_current_versions(runtime: &RuntimePaths) -> HashMap<String, String> {
    let mut versions = HashMap::new();
    let data_scope = runtime.data_root.join("plugins/@dshplusplus");
    let scope = if data_scope.is_dir() {
        data_scope
    } else if let Some(dir) = &runtime.plugins_dir {
        dir.clone()
    } else {
        return versions;
    };
    for package in [
        "multimodal",
        "multimodal-llm",
        "multimodal-router",
        "tool-media-inspect",
        "bundle-plus",
    ] {
        let manifest_path = scope.join(package).join("package.json");
        if let Ok(text) = fs::read_to_string(&manifest_path) {
            if let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(version) = manifest.get("version").and_then(serde_json::Value::as_str) {
                    versions.insert(package.to_string(), version.to_string());
                }
            }
        }
    }
    versions
}

/// 本地 DSH CLI 版本（`node <cli> --version` 首行）。无法获取时返回 None。
fn local_dsh_version(runtime: &RuntimePaths, config: &StoredConfig) -> Option<String> {
    let cli = effective_dsh_cli(runtime, config)?;
    let mut command = if is_script_cli(&cli) {
        let mut command = std::process::Command::new(runtime.node.as_ref()?);
        command.arg(&cli);
        command
    } else {
        std::process::Command::new(&cli)
    };
    let output = command.arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
}

/// npm registry 上的 DSH 最新版本。
fn npm_dsh_latest() -> Option<String> {
    let bytes = http_get_bytes("https://registry.npmjs.org/@deepseek-ai/dsh/latest").ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    value
        .get("version")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
}

/// 检查更新：本地暂存 + 远程清单（app/plugins/mca/dsh 四类组件状态）。
#[tauri::command]
fn check_for_update(app: tauri::AppHandle) -> Result<UpdateCheckResult, String> {
    let state = app.state::<AppState>();
    let config = state.config.lock().map_err(|_| "配置状态锁已损坏")?.clone();
    let runtime = state.runtime.clone();
    let exe_dir = std::env::current_exe()
        .map_err(|error| error.to_string())?
        .parent()
        .ok_or("无法解析程序目录")?
        .to_path_buf();
    let staged = exe_dir.join("DSHPlusPlus.update.exe");

    let mut components: Vec<ComponentUpdate> = Vec::new();

    // 1) 远程更新源（配置了才用）。
    let remote_url = config.update_url.trim().to_string();
    let manifest = if remote_url.is_empty() {
        None
    } else {
        match fetch_update_manifest(&remote_url) {
            Ok(manifest) => Some(manifest),
            Err(error) => {
                components.push(ComponentUpdate {
                    name: "app".into(),
                    current: Some(VERSION.into()),
                    latest: None,
                    available: false,
                    note: format!("清单获取失败：{error}"),
                });
                None
            }
        }
    };

    // app（DSH++ 自身）：清单有更新则立即下载暂存（配合"更新到新版.cmd"）。
    let app_artifact = manifest.as_ref().and_then(|m| m.app.clone());
    if let Some(artifact) = &app_artifact {
        let app_available = artifact.version != VERSION;
        if app_available && !artifact.url.is_empty() {
            match http_get_bytes(&artifact.url) {
                Ok(bytes) => {
                    write_atomic(&staged, &bytes)
                        .map_err(|error| format!("暂存更新失败：{error}"))?;
                    components.push(ComponentUpdate {
                        name: "app".into(),
                        current: Some(VERSION.into()),
                        latest: Some(artifact.version.clone()),
                        available: true,
                        note: "已下载到 DSHPlusPlus.update.exe；退出后运行“更新到新版.cmd”即可生效".into(),
                    });
                }
                Err(error) => components.push(ComponentUpdate {
                    name: "app".into(),
                    current: Some(VERSION.into()),
                    latest: Some(artifact.version.clone()),
                    available: false,
                    note: error,
                }),
            }
        } else if staged.is_file() {
            components.push(ComponentUpdate {
                name: "app".into(),
                current: Some(VERSION.into()),
                latest: Some(artifact.version.clone()),
                available: false,
                note: "与清单一致；发现本地暂存的更新文件，退出后运行“更新到新版.cmd”".into(),
            });
        } else {
            components.push(ComponentUpdate {
                name: "app".into(),
                current: Some(VERSION.into()),
                latest: Some(artifact.version.clone()),
                available: false,
                note: "已是最新版本".into(),
            });
        }
    }

    // 插件：对比本地 plugins 版本与清单 packages。
    if let Some(plugins) = manifest.as_ref().and_then(|m| m.plugins.clone()) {
        let local = plugin_current_versions(&runtime);
        for (package, latest) in &plugins.packages {
            let current = local.get(package).cloned();
            let available = current.as_deref() != Some(latest.as_str());
            components.push(ComponentUpdate {
                name: package.clone(),
                current,
                latest: Some(latest.clone()),
                available,
                note: if available {
                    "点击「更新插件与 MCA」下载，重启 DSH 后生效".into()
                } else {
                    "已是最新版本".into()
                },
            });
        }
    }

    // MCA：清单提供最新版本（本地无版本资源，更新即下载替换）。
    if let Some(artifact) = manifest.as_ref().and_then(|m| m.mca.clone()) {
        components.push(ComponentUpdate {
            name: "mca".into(),
            current: None,
            latest: Some(artifact.version.clone()),
            available: !artifact.url.is_empty(),
            note: "点击「更新插件与 MCA」下载替换，重启后生效".into(),
        });
    }

    // DSH（上游）：本地 CLI 版本 vs npm registry latest。
    let dsh_installed = local_dsh_version(&runtime, &config);
    let dsh_latest = npm_dsh_latest();
    components.push(ComponentUpdate {
        name: "dsh".into(),
        current: dsh_installed.clone(),
        latest: dsh_latest.clone(),
        available: match (&dsh_installed, &dsh_latest) {
            (Some(current), Some(latest)) => current != latest,
            _ => false,
        },
        note: match (&dsh_installed, &dsh_latest) {
            (Some(current), Some(latest)) if current != latest => {
                format!("上游有新版 {latest}：npm install -g @deepseek-ai/dsh@latest")
            }
            (Some(_), Some(_)) => "已是最新版本".into(),
            (None, _) => "未安装 DSH（或无法读取版本）".into(),
            (Some(current), None) => format!("本地 {current}；npm registry 查询失败（网络？）"),
        },
    });

    // 2) 本地暂存检测（未配置远程源时）。
    if manifest.is_none() && staged.is_file() {
        components.push(ComponentUpdate {
            name: "app".into(),
            current: Some(VERSION.into()),
            latest: None,
            available: true,
            note: "发现本地暂存的新版本（DSHPlusPlus.update.exe）。请退出后运行“更新到新版.cmd”。".into(),
        });
    }

    let app_update = components
        .iter()
        .find(|component| component.name == "app" && component.available);
    let message = match (&app_update, manifest) {
        (Some(update), _) => format!(
            "发现新版本 {}（已下载），请退出后运行“更新到新版.cmd”。",
            update.latest.as_deref().unwrap_or("DSH++")
        ),
        (None, Some(_)) => format!("DSH++ 已是最新（{VERSION}）。"),
        (None, None) if staged.is_file() => {
            "发现本地暂存更新，请退出后运行“更新到新版.cmd”。".into()
        }
        (None, None) => format!("当前已是最新版本（{VERSION}）。未发现待安装更新。"),
    };
    Ok(UpdateCheckResult {
        available: app_update.is_some(),
        version: app_update.and_then(|update| update.latest.clone()),
        message,
        components,
    })
}

/// 应用插件与 MCA 更新：按清单下载插件 tarball 到 data_root/plugins、
/// MCA 到 data_root/mca，重启 DSH/MCA 后生效。
#[tauri::command]
fn apply_updates(app: tauri::AppHandle) -> Result<String, String> {
    let state = app.state::<AppState>();
    let config = state.config.lock().map_err(|_| "配置状态锁已损坏")?.clone();
    let runtime = state.runtime.clone();
    let remote_url = config.update_url.trim().to_string();
    if remote_url.is_empty() {
        return Err("未配置远程更新源（运行环境页 → 更新源 URL）。插件与 MCA 无法在线更新。".into());
    }
    let manifest = fetch_update_manifest(&remote_url)?;
    let mut applied: Vec<String> = Vec::new();

    // 插件：下载 tarball → 解压到 data_root/plugins/@dshplusplus/<name>
    if let Some(plugins) = &manifest.plugins {
        let local = plugin_current_versions(&runtime);
        let destination = runtime.data_root.join("plugins/@dshplusplus");
        fs::create_dir_all(&destination).map_err(|error| error.to_string())?;
        for (package, latest) in &plugins.packages {
            let current = local.get(package).cloned();
            if current.as_deref() == Some(latest.as_str()) {
                continue;
            }
            let tarball_url = format!(
                "{}{}-{}.tgz",
                plugins.url_prefix.trim_end_matches('/'),
                package.replace("@dshplusplus/", "dshplusplus-"),
                latest
            );
            let bytes = http_get_bytes(&tarball_url)?;
            let cache = runtime.data_root.join("plugins-cache");
            fs::create_dir_all(&cache).map_err(|error| error.to_string())?;
            let tarball_path = cache.join(format!(
                "{}-{}.tgz",
                package.replace("@dshplusplus/", "dshplusplus-"),
                latest
            ));
            write_atomic(&tarball_path, &bytes)?;
            // 用系统 tar 解压（package/ 目录）并覆盖目标。
            let extract_dir = cache.join(format!("extract-{package}-{latest}"));
            if extract_dir.exists() {
                fs::remove_dir_all(&extract_dir).map_err(|error| error.to_string())?;
            }
            fs::create_dir_all(&extract_dir).map_err(|error| error.to_string())?;
            let status = std::process::Command::new("tar")
                .args(["-xzf"])
                .arg(&tarball_path)
                .args(["-C"])
                .arg(&extract_dir)
                .status()
                .map_err(|error| format!("解压失败（tar 不可用）：{error}"))?;
            if !status.success() {
                return Err(format!("解压插件包失败：{package}"));
            }
            let package_dir = extract_dir.join("package");
            if !package_dir.is_dir() {
                return Err(format!("插件包结构异常：{package}"));
            }
            let target = destination.join(package);
            if target.exists() {
                fs::remove_dir_all(&target).map_err(|error| error.to_string())?;
            }
            fs::rename(&package_dir, &target).map_err(|error| error.to_string())?;
            let _ = fs::remove_dir_all(&extract_dir);
            applied.push(format!("{package} → {latest}"));
        }
    }

    // MCA：下载到 data_root/mca/mca-runtime.exe（discover 优先采用）。
    if let Some(artifact) = &manifest.mca {
        if !artifact.url.is_empty() {
            let bytes = http_get_bytes(&artifact.url)?;
            let mca_dir = runtime.data_root.join("mca");
            fs::create_dir_all(&mca_dir).map_err(|error| error.to_string())?;
            write_atomic(&mca_dir.join("mca-runtime.exe"), &bytes)?;
            applied.push(format!("mca → {}", artifact.version));
        }
    }

    if applied.is_empty() {
        return Ok("插件与 MCA 均已是最新版本".into());
    }
    Ok(format!(
        "已更新：{}。重启 DSH 后插件生效；MCA 会在下次启动时自动使用新版本。",
        applied.join("，")
    ))
}

/// 打开 DeepSeek Harness 获取页面（本机未安装 DSH 时的引导）。
#[tauri::command]
fn open_dsh_guide() -> Result<(), String> {
    let url = "https://github.com/deepseek-ai/DeepSeekHarness#readme";
    std::process::Command::new("cmd")
        .args(["/c", "start", "", url])
        .spawn()
        .map_err(|error| format!("无法打开浏览器：{error}"))?;
    Ok(())
}

/// 浏览器可执行文件：Program Files / Program Files (x86) / LOCALAPPDATA
/// 下的标准安装位置。
fn browser_executable(browser: &str) -> Option<PathBuf> {
    let program_files = std::env::var_os("ProgramFiles").map(PathBuf::from);
    let program_files_x86 = std::env::var_os("ProgramFiles(x86)").map(PathBuf::from);
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let mut candidates: Vec<PathBuf> = Vec::new();
    match browser {
        "edge" => {
            if let Some(path) = &program_files_x86 {
                candidates.push(path.join(r"Microsoft\Edge\Application\msedge.exe"));
            }
            if let Some(path) = &program_files {
                candidates.push(path.join(r"Microsoft\Edge\Application\msedge.exe"));
            }
        }
        _ => {
            if let Some(path) = &program_files {
                candidates.push(path.join(r"Google\Chrome\Application\chrome.exe"));
            }
            if let Some(path) = &program_files_x86 {
                candidates.push(path.join(r"Google\Chrome\Application\chrome.exe"));
            }
            if let Some(path) = &local {
                candidates.push(path.join(r"Google\Chrome\Application\chrome.exe"));
            }
        }
    }
    candidates.into_iter().find(|path| path.is_file())
}

/// 目标浏览器进程是否正在运行（tasklist 按映像名过滤）。
fn browser_process_running(browser: &str) -> bool {
    let image = match browser {
        "edge" => "msedge.exe",
        _ => "chrome.exe",
    };
    let mut command = Command::new("tasklist");
    command.args(["/FI", &format!("IMAGENAME eq {image}"), "/FO", "CSV", "/NH"]);
    hide_console(&mut command);
    match command.output() {
        Ok(output) => String::from_utf8_lossy(&output.stdout)
            .to_lowercase()
            .contains(image),
        Err(_) => false,
    }
}

/// 轮询共享桥连接状态，最多 timeout（浏览器启动 + 扩展 service worker
/// 唤醒 + native host 拉起需要数秒）。
fn wait_shared_connected(timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if fetch_shared_connected() {
            return true;
        }
        thread::sleep(Duration::from_millis(500));
    }
    false
}

/// 回退引导：打开目标浏览器的扩展管理页，并把扩展目录复制到剪贴板，
/// 把 Chromium 安全边界要求的唯一一次手动操作压缩成“粘贴 + 确认”。
fn extension_guidance(browser: &str, executable: &Path, destination: &Path) -> String {
    let page = match browser {
        "edge" => "edge://extensions",
        _ => "chrome://extensions",
    };
    let browser_label = match browser {
        "edge" => "Edge",
        _ => "Chrome",
    };
    let _ = Command::new(executable).arg(page).spawn();
    let destination_text = path_string(destination);
    let mut clipboard = Command::new("clip");
    clipboard
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide_console(&mut clipboard);
    if let Ok(mut child) = clipboard.spawn() {
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(destination_text.as_bytes());
        }
        let _ = child.wait();
    }
    format!(
        "{browser_label} 扩展页面已打开，扩展目录已复制到剪贴板。请开启“开发者模式”，点击“加载已解压的扩展程序”，粘贴目录并确认；加载一次后永久生效。"
    )
}

/// 一键安装/修复/连接 chromeUse 扩展。按目标浏览器的实时安装态分派：
/// - installed：拉起（或复用）浏览器让桥自动连接；
/// - 其余三态：浏览器未运行时以 --load-extension 拉起（旧版浏览器直接
///   生效）；Chrome/Edge 137+ 拦截命令行加载，轮询桥连接失败后回退
///   「扩展页 + 剪贴板」引导，加载一次后永久持久化。
#[tauri::command]
fn install_chrome_extension(
    state: State<'_, AppState>,
    browser: Option<String>,
) -> Result<String, String> {
    let browser = match browser.as_deref() {
        Some("edge") => "edge",
        _ => "chrome",
    };
    let browser_label = match browser {
        "edge" => "Edge",
        _ => "Chrome",
    };
    // 桥连接的前提是网关在线；未运行时先拉起（chromeUse 启动流程同款）。
    if !port_open("127.0.0.1", BROWSER_PORT) {
        let config = state
            .config
            .lock()
            .map_err(|_| "配置状态锁已损坏")?
            .clone();
        start_browser(&state, &config)?;
    }
    let destination = prepare_chrome_extension(&state)?;
    let exe = browser_executable(browser)
        .ok_or_else(|| format!("未找到{browser_label}，请先安装{browser_label}"))?;

    let user_data = browser_user_data_dir(browser);
    let expected = expected_extension_manifest(&state.runtime);
    let (status, _, _) = match &user_data {
        Some(dir) if dir.is_dir() => scan_browser_extension(dir, expected.as_ref()),
        _ => (ExtensionInstallState::NotInstalled, None, None),
    };
    let running = browser_process_running(browser);
    let message = match status {
        ExtensionInstallState::Installed => {
            if !running {
                let _ = Command::new(&exe).spawn();
            }
            if wait_shared_connected(Duration::from_secs(8)) {
                format!("{browser_label} 扩展已连接，可以在已打开的标签页中使用了。")
            } else {
                format!("{browser_label} 扩展已安装；浏览器启动后扩展会自动连接，稍候状态会更新为“已连接”。")
            }
        }
        _ => {
            if running {
                // 命令行参数对已运行实例无效，直接引导。
                extension_guidance(browser, &exe, &destination)
            } else {
                let _ = Command::new(&exe)
                    .arg(format!("--load-extension={}", path_string(&destination)))
                    .spawn();
                if wait_shared_connected(Duration::from_secs(8)) {
                    format!("{browser_label} 扩展已加载并连接。若状态仍显示未安装，请在扩展管理页“加载已解压”一次以持久化。")
                } else {
                    extension_guidance(browser, &exe, &destination)
                }
            }
        }
    };
    // 安装动作可能改变 profile 记录，清空扫描缓存让 UI 立即翻转状态。
    if let Ok(mut cache) = state.extension_status.lock() {
        *cache = None;
    }
    Ok(message)
}

/// 双浏览器扩展安装状态（强制全量扫描，绕过缓存）。
#[tauri::command]
fn chrome_extension_status(state: State<'_, AppState>) -> Result<ChromeExtensionView, String> {
    Ok(scan_extension_statuses(&state))
}

#[tauri::command]
fn enable_computer_provider(state: State<'_, AppState>) -> Result<AppSnapshot, String> {
    let config = state.config.lock().map_err(|_| "配置锁已损坏")?.clone();
    if !config.enable_mca {
        return Err("请先在扩展能力中启用 MCA 能力层".into());
    }
    if !port_open("127.0.0.1", MCA_PORT) {
        return Err("MCA 尚未运行，请先点击“启动 DSH”启动能力层".into());
    }
    // 先无条件启用桌面 Provider：不能依赖配置里的能力勾选——开关被禁用
    // 正是因为 Provider 未启用，依赖勾选会形成死锁。
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .build()
        .into();
    enable_desktop_provider(&agent, MCA_PORT)?;
    configure_mca_route(&state, &config, true)?;
    snapshot(&state)
}

/// `mca-runtime serve` 的固定参数（数据目录由调用方追加在 --data 之后）。
/// --agent-base-url 指向自己：路由健康探测不再落到外部 MCA 实例
/// （如独立 MCA Control Center 的 8766）上。
fn mca_serve_args(port: u16) -> Vec<String> {
    vec![
        "serve".into(),
        "--host".into(),
        "127.0.0.1".into(),
        "--port".into(),
        port.to_string(),
        "--agent-base-url".into(),
        format!("http://127.0.0.1:{port}"),
        "--data".into(),
    ]
}

fn start_mca(state: &AppState, config: &StoredConfig) -> Result<(), String> {
    if !config.enable_mca {
        return Ok(());
    }
    let reused = port_open("127.0.0.1", MCA_PORT);
    // 先生成 agent-shims（MCA 通过 PATH 上的 `deepseek` 命令探测 deepseek-tui）。
    let shim_dir = ensure_agent_shims(state, config);
    if !reused {
        let binary = state
            .runtime
            .mca
            .as_ref()
            .ok_or("未找到 MCA Sidecar；请重新构建 Full 便携包，或关闭 MCA")?;
        let data = state.runtime.data_root.join("mca-data");
        fs::create_dir_all(&data).map_err(|error| error.to_string())?;
        let (stdout, stderr) = log_files(&state.runtime, "mca")?;
        let mut command = Command::new(binary);
        let mut args = mca_serve_args(MCA_PORT);
        args.push(data.to_string_lossy().into_owned());
        command
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        command.env("MCA_DATA_ROOT", &data);
        // 把 agent-shims 目录前置到 MCA 的 PATH：MCA 的 Agent 探测
        // （shutil.which）使用进程环境，优先命中我们生成的 deepseek.cmd。
        if let Some(dir) = shim_dir.as_ref() {
            let existing = std::env::var_os("PATH").unwrap_or_default();
            let joined = format!(
                "{};{}",
                path_string(dir),
                existing.to_string_lossy()
            );
            command.env("PATH", joined);
        }
        // 跟随系统代理：MCA 的 httpx 请求（网页/在线媒体）与 yt-dlp 子进程
        // 都读取 HTTP_PROXY/HTTPS_PROXY；NO_PROXY 豁免本机回环地址。
        for (name, value) in system_proxy_env() {
            command.env(name, value);
        }
        hide_console(&mut command);
        let child = command
            .spawn()
            .map_err(|error| format!("无法启动 MCA：{error}"))?;
        #[cfg(target_os = "windows")]
        let job_handle = assign_kill_on_close_job(&child);
        {
            let mut service = state.mca.lock().map_err(|_| "MCA 状态锁已损坏")?;
            service.child = Some(child);
            #[cfg(target_os = "windows")]
            {
                service.job_handle = job_handle;
            }
            service.state = ServiceState::Starting;
            service.message = "等待 MCA API 就绪".into();
        }
        if !wait_for_port("127.0.0.1", MCA_PORT, Duration::from_secs(18)) {
            let mut service = state.mca.lock().map_err(|_| "MCA 状态锁已损坏")?;
            service.state = ServiceState::Error;
            service.message = "MCA 在 18 秒内没有就绪；请查看诊断日志".into();
            return Err(service.message.clone());
        }
    } else {
        let mut service = state.mca.lock().map_err(|_| "MCA 状态锁已损坏")?;
        service.state = ServiceState::Starting;
        service.message = format!("正在连接本机 MCA（端口 {MCA_PORT}）");
    }
    configure_mca_route(state, config, reused)
}

fn random_suffix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{nanos}")
}

/// 把旧会话的模型选择从 llm-deepseek 家族平移到 deepseek-plus（图片适配）。
/// DSH 的 prompt 校验读取的是“会话级”模型选择；旧会话持久化的
/// deepseek-official 会继续拦截图片附件。该函数幂等：只处理
/// current.provider 属于 deepseek-official/deepseek 的会话，其余跳过。
fn migrate_sessions_to_plus(host: &str, port: u16) {
    let base = format!("http://{host}:{port}/api/");
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(8)))
        .build()
        .into();
    let rpc = |method: &str, payload: serde_json::Value| -> Option<serde_json::Value> {
        let body = json!({
            "type": "client-request",
            "rpcId": format!("dshplusplus-{}-{}", std::process::id(), random_suffix()),
            "method": method,
            "payload": payload,
        });
        let mut response = agent
            .post(&format!("{base}{method}"))
            .send_json(body)
            .ok()?;
        let text = response.body_mut().read_to_string().ok()?;
        let parsed: serde_json::Value = serde_json::from_str(&text).ok()?;
        let result = parsed.get("result")?.clone();
        if result.get("ok").and_then(serde_json::Value::as_bool) != Some(true) {
            return None;
        }
        Some(result)
    };
    let Some(list) = rpc("session.list", json!({})) else {
        return;
    };
    let Some(items) = list
        .get("value")
        .and_then(|value| value.get("items"))
        .and_then(serde_json::Value::as_array)
    else {
        return;
    };
    for item in items {
        let Some(session_id) = item.get("sessionId").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let Some(models) = rpc("session.models", json!({ "sessionId": session_id })) else {
            continue;
        };
        let Some(current) = models.get("value").and_then(|value| value.get("current")) else {
            continue;
        };
        let provider = current
            .get("provider")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if provider != "deepseek-official" && provider != "deepseek" {
            continue;
        }
        let model = current
            .get("model")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if model.is_empty() {
            continue;
        }
        let mut payload = json!({
            "sessionId": session_id,
            "provider": "deepseek-plus",
            "model": model,
        });
        if let Some(effort) = current
            .get("reasoningEffort")
            .and_then(serde_json::Value::as_str)
        {
            payload["reasoningEffort"] = serde_json::Value::String(effort.into());
        }
        rpc("session.selectModel", payload);
    }
}

/// CLI 是否是 Node 脚本（.js/.mjs/.cjs——用内置 node 执行）。
fn is_script_cli(cli: &Path) -> bool {
    cli.extension().is_some_and(|ext| {
        ext.eq_ignore_ascii_case("js")
            || ext.eq_ignore_ascii_case("mjs")
            || ext.eq_ignore_ascii_case("cjs")
    })
}

/// 本次启动实际使用的 DSH CLI：显式配置（存在时）优先，否则用自动发现结果。
fn effective_dsh_cli(runtime: &RuntimePaths, config: &StoredConfig) -> Option<PathBuf> {
    if !config.dsh_cli.trim().is_empty() {
        if let Some(cli) = resolve_dsh_cli_candidate(Path::new(config.dsh_cli.trim())) {
            return Some(cli);
        }
        eprintln!(
            "[dshplusplus] 配置的 DSH CLI 不存在（{}），回退到自动发现",
            config.dsh_cli
        );
    }
    runtime.dsh_cli.clone()
}

#[tauri::command]
fn resolve_dsh_cli_path(path: String) -> Result<String, String> {
    resolve_dsh_cli_candidate(Path::new(path.trim()))
        .as_deref()
        .map(path_string)
        .ok_or_else(|| {
            "所选位置中未找到 DSH CLI；请选择 DeepSeekHarness 仓库目录、安装目录或 bin.js/dsh.cmd/dsh.exe".into()
        })
}

#[tauri::command]
fn detect_dsh_cli() -> Option<String> {
    std::env::var_os("DSHPLUSPLUS_DSH_CLI")
        .and_then(|value| resolve_dsh_cli_candidate(&PathBuf::from(value)))
        .or_else(find_dsh_on_path)
        .or_else(find_dsh_npm_global)
        .or_else(find_dsh_source_checkout)
        .as_deref()
        .map(path_string)
}

fn start_dsh(state: &AppState, config: &StoredConfig) -> Result<(), String> {
    if port_open(&config.dsh_host, config.dsh_port) {
        if is_dsh_endpoint(&config.dsh_host, config.dsh_port) {
            let mut service = state.dsh.lock().map_err(|_| "DSH 状态锁已损坏")?;
            service.state = ServiceState::Running;
            service.message = format!("已连接现有 DSH · {}:{}", config.dsh_host, config.dsh_port);
            return Ok(());
        }
        return Err(format!(
            "端口 {} 已被其他程序占用，且不是 DSH。若是上次退出残留的 DSH 进程，请在任务管理器中结束占用该端口的进程后重试",
            config.dsh_port
        ));
    }
    let node = state.runtime.node.as_ref().ok_or("未找到 Node 运行时")?;
    let cli = effective_dsh_cli(&state.runtime, config)
        .ok_or("未找到本地 DSH。请先安装 DeepSeek Harness（npm i -g @deepseek-ai/dsh 或下载官方安装包），或在「运行环境」中指定 DSH CLI 路径，然后重新点击「启动 DSH」。")?;
    // 使用标准 home 时，把旧便携 home（.portable/dsh-home）的数据一次性并入
    // 标准 home（幂等；失败不阻塞启动，见诊断日志）。
    if let Err(error) = migrate_portable_home_data(&state.runtime) {
        eprintln!("[dshplusplus] 便携数据迁移失败（可忽略）：{error}");
    }
    let home = effective_dsh_home(&state.runtime);
    materialize_dsh_config(&state.runtime, config, &home)?;
    let workspace = if config.workspace.is_empty() {
        state.runtime.data_root.clone()
    } else {
        PathBuf::from(&config.workspace)
    };
    let (stdout, stderr) = log_files(&state.runtime, "dsh")?;
    // CLI 形态适配：bin.js 用内置 node 跑；.cmd/.bat 是 npm shim，用 cmd /c；
    // .exe 直接执行。
    let mut command = if is_script_cli(&cli) {
        let mut command = Command::new(node);
        command.arg(&cli);
        command
    } else if cli
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat"))
    {
        let mut command = Command::new("cmd");
        command.arg("/c").arg(&cli);
        command
    } else {
        Command::new(&cli)
    };
    command
        .args([
            "--profile",
            "dshplusplus",
            "--host",
            &config.dsh_host,
            "--port",
            &config.dsh_port.to_string(),
        ])
        .current_dir(workspace)
        .env("DSH_HOME", &home)
        .env("DSH_TELEMETRY_DISABLED", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    if let Some(secret) = config.vision_secret.as_deref() {
        command.env("DSHPLUSPLUS_VISION_API_KEY", unprotect_secret(secret)?);
    }
    hide_console(&mut command);
    let child = command
        .spawn()
        .map_err(|error| format!("无法启动 DSH：{error}"))?;
    #[cfg(target_os = "windows")]
    let job_handle = assign_kill_on_close_job(&child);
    {
        let mut service = state.dsh.lock().map_err(|_| "DSH 状态锁已损坏")?;
        service.child = Some(child);
        #[cfg(target_os = "windows")]
        {
            service.job_handle = job_handle;
        }
        service.state = ServiceState::Starting;
        service.message = "正在组合 DSH++ Profile".into();
    }
    if !wait_for_dsh(&config.dsh_host, config.dsh_port, Duration::from_secs(60)) {
        let mut service = state.dsh.lock().map_err(|_| "DSH 状态锁已损坏")?;
        service.refresh(&config.dsh_host, config.dsh_port);
        if port_open(&config.dsh_host, config.dsh_port) {
            service.state = ServiceState::Error;
            service.message = format!("端口 {} 已监听，但返回的不是 DSH 页面", config.dsh_port);
        } else if !matches!(service.state, ServiceState::Error) {
            service.state = ServiceState::Error;
            service.message = "DSH 在 60 秒内没有就绪；请查看诊断日志，或停止占用 18760 端口的残留进程后重试".into();
        }
        return Err(service.message.clone());
    }
    let mut service = state.dsh.lock().map_err(|_| "DSH 状态锁已损坏")?;
    service.state = ServiceState::Running;
    service.message = format!(
        "DSH++ Profile · PID {}",
        service.child.as_ref().map(Child::id).unwrap_or(0)
    );
    Ok(())
}

fn snapshot(state: &AppState) -> Result<AppSnapshot, String> {
    let config = state.config.lock().map_err(|_| "配置状态锁已损坏")?.clone();
    let mut dsh = state.dsh.lock().map_err(|_| "DSH 状态锁已损坏")?;
    let mut mca = state.mca.lock().map_err(|_| "MCA 状态锁已损坏")?;
    let mut browser = state.browser.lock().map_err(|_| "浏览器状态锁已损坏")?;
    dsh.refresh(&config.dsh_host, config.dsh_port);
    mca.refresh("127.0.0.1", MCA_PORT);
    browser.refresh("127.0.0.1", BROWSER_PORT);
    // MCA 运行时读取 deepseek-tui 路由能力/健康（供 UI 动态化开关）。
    let mca_route = if matches!(mca.state, ServiceState::Running) && config.enable_mca {
        fetch_mca_route()
    } else {
        None
    };
    // MCA 工具级健康（能力卡片展示）。
    let mca_providers = if matches!(mca.state, ServiceState::Running) && config.enable_mca {
        fetch_mca_providers()
    } else {
        Vec::new()
    };
    Ok(AppSnapshot {
        version: VERSION,
        config: ConfigView::from(&config),
        runtime: RuntimeInfo {
            portable: state.runtime.portable,
            data_root: path_string(&state.runtime.data_root),
            dsh_home: Some(path_string(&effective_dsh_home(&state.runtime))),
            dsh_cli: effective_dsh_cli(&state.runtime, &config).as_deref().map(path_string),
            node_binary: state.runtime.node.as_deref().map(path_string),
            mca_binary: state.runtime.mca.as_deref().map(path_string),
            browser_gateway: state.runtime.browser_gateway.as_deref().map(path_string),
        },
        dsh_state: dsh.state,
        dsh_url: format!("http://{}:{}", config.dsh_host, config.dsh_port),
        dsh_pid: dsh.child.as_ref().map(Child::id),
        dsh_message: dsh.message.clone(),
        mca_state: mca.state,
        mca_url: config
            .enable_mca
            .then(|| format!("http://127.0.0.1:{MCA_PORT}")),
        mca_pid: mca.child.as_ref().map(Child::id),
        mca_message: if !config.enable_mca {
            "已在配置中关闭".into()
        } else {
            mca.message.clone()
        },
        mca_route,
        mca_providers,
        browser_state: browser.state,
        browser_pid: browser.child.as_ref().map(Child::id),
        browser_message: if !config.enable_browser && !config.enable_chrome_use {
            "已在配置中关闭".into()
        } else {
            browser.message.clone()
        },
        chrome_extension: cached_extension_status(state),
    })
}

#[tauri::command]
fn get_snapshot(app: tauri::AppHandle) -> Result<AppSnapshot, String> {
    let state = app.state::<AppState>();
    let data = snapshot(&state)?;
    sync_dsh_window(&app, data.dsh_state);
    Ok(data)
}

#[tauri::command]
fn refresh_status(app: tauri::AppHandle) -> Result<AppSnapshot, String> {
    let state = app.state::<AppState>();
    let data = snapshot(&state)?;
    sync_dsh_window(&app, data.dsh_state);
    Ok(data)
}

#[tauri::command]
fn save_config(input: ConfigInput, app: tauri::AppHandle) -> Result<AppSnapshot, String> {
    let state = app.state::<AppState>();
    validate_config(&input)?;
    let mut current = state.config.lock().map_err(|_| "配置状态锁已损坏")?;
    let deepseek_secret = match input
        .deepseek_api_key
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        Some(value) => Some(protect_secret(value)?),
        None => current.deepseek_secret.clone(),
    };
    let vision_secret = match input
        .vision_api_key
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        Some(value) => Some(protect_secret(value)?),
        None => current.vision_secret.clone(),
    };
    let dsh_cli = if input.dsh_cli.trim().is_empty() {
        String::new()
    } else {
        path_string(
            &resolve_dsh_cli_candidate(Path::new(input.dsh_cli.trim()))
                .ok_or("所选位置中未找到 DSH CLI")?,
        )
    };
    *current = StoredConfig {
        dsh_host: input.dsh_host,
        dsh_port: input.dsh_port,
        workspace: input.workspace,
        dsh_cli,
        update_url: input.update_url,
        auto_start_dsh: input.auto_start_dsh,
        auto_open_dsh_window: input.auto_open_dsh_window,
        enable_mca: input.enable_mca,
        enable_browser: input.enable_browser,
        enable_chrome_use: input.enable_chrome_use,
        mca_image: input.mca_image,
        mca_video: input.mca_video,
        mca_audio: input.mca_audio,
        mca_document: input.mca_document,
        mca_web: input.mca_web,
        mca_computer_observe: input.mca_computer_observe,
        mca_computer_act: input.mca_computer_act,
        deepseek_base_url: input.deepseek_base_url,
        deepseek_model: input.deepseek_model,
        deepseek_secret,
        vision_provider: input.vision_provider,
        vision_base_url: input.vision_base_url,
        vision_model: input.vision_model,
        vision_api: input.vision_api,
        vision_secret,
        enable_multimodal: input.enable_multimodal,
    };
    let serialized = serde_json::to_vec_pretty(&*current).map_err(|error| error.to_string())?;
    write_atomic(&config_path(&state.runtime), &serialized)?;
    let home = effective_dsh_home(&state.runtime);
    materialize_dsh_config(&state.runtime, &current, &home)?;
    let updated = current.clone();
    drop(current);
    if port_open("127.0.0.1", MCA_PORT) {
        let _ = configure_mca_route(&state, &updated, true);
    }
    // 浏览器网关：开关打开且未运行时拉起；全部关闭时停止。
    let browser_should_run = updated.enable_browser || updated.enable_chrome_use;
    let browser_running = {
        let mut browser = state.browser.lock().map_err(|_| "浏览器状态锁已损坏")?;
        browser.refresh("127.0.0.1", BROWSER_PORT);
        matches!(
            browser.state,
            ServiceState::Running | ServiceState::Starting
        )
    };
    if browser_should_run && !browser_running {
        let _ = start_browser(&state, &updated);
    } else if !browser_should_run && browser_running {
        unregister_chrome_native_host();
        state
            .browser
            .lock()
            .map_err(|_| "浏览器状态锁已损坏")?
            .stop();
    }
    let data = snapshot(&state)?;
    if updated.auto_open_dsh_window && matches!(data.dsh_state, ServiceState::Running) {
        let _ = open_dsh_window(&app);
    }
    Ok(data)
}

#[tauri::command]
fn start_services(app: tauri::AppHandle) -> Result<AppSnapshot, String> {
    let state = app.state::<AppState>();
    let config = state.config.lock().map_err(|_| "配置状态锁已损坏")?.clone();
    {
        let mut dsh = state.dsh.lock().map_err(|_| "DSH 状态锁已损坏")?;
        dsh.refresh(&config.dsh_host, config.dsh_port);
        if matches!(dsh.state, ServiceState::Running | ServiceState::Starting) {
            return snapshot(&state);
        }
        dsh.state = ServiceState::Starting;
        dsh.message = "启动任务已提交".into();
    }
    if config.enable_mca {
        let mut mca = state.mca.lock().map_err(|_| "MCA 状态锁已损坏")?;
        mca.refresh("127.0.0.1", MCA_PORT);
        if !matches!(mca.state, ServiceState::Running | ServiceState::Starting) {
            mca.state = ServiceState::Starting;
            mca.message = "准备内容与浏览器能力".into();
        }
    }
    let worker = app.clone();
    let auto_open = config.auto_open_dsh_window;
    thread::spawn(move || {
        let state = worker.state::<AppState>();
        if config.enable_mca {
            let _ = start_mca(&state, &config);
        }
        if config.enable_browser || config.enable_chrome_use {
            let _ = start_browser(&state, &config);
        }
        if let Err(error) = start_dsh(&state, &config) {
            if let Ok(mut dsh) = state.dsh.lock() {
                dsh.state = ServiceState::Error;
                dsh.message = error;
            }
        } else {
            // 旧会话的模型选择平移到 deepseek-plus（图片发送适配）。
            migrate_sessions_to_plus(&config.dsh_host, config.dsh_port);
            if auto_open {
                let handle = worker.clone();
                let window_handle = handle.clone();
                let _ = handle.run_on_main_thread(move || {
                    let _ = open_dsh_window(&window_handle);
                });
            }
        }
    });
    snapshot(&state)
}

#[tauri::command]
fn stop_services(app: tauri::AppHandle) -> Result<AppSnapshot, String> {
    let state = app.state::<AppState>();
    unregister_chrome_native_host();
    state.dsh.lock().map_err(|_| "DSH 状态锁已损坏")?.stop();
    state.mca.lock().map_err(|_| "MCA 状态锁已损坏")?.stop();
    state
        .browser
        .lock()
        .map_err(|_| "浏览器状态锁已损坏")?
        .stop();
    sync_dsh_window(&app, ServiceState::Stopped);
    snapshot(&state)
}

#[cfg(target_os = "windows")]
fn open_external_url(url: &str) -> Result<(), String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let operation: Vec<u16> = OsStr::new("open").encode_wide().chain(Some(0)).collect();
    let target: Vec<u16> = OsStr::new(url).encode_wide().chain(Some(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result as isize <= 32 {
        return Err(format!("无法调用系统浏览器（ShellExecute={result:?}）"));
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn open_external_url(url: &str) -> Result<(), String> {
    let command = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    Command::new(command)
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn open_dsh(state: State<'_, AppState>) -> Result<(), String> {
    let config = state.config.lock().map_err(|_| "配置状态锁已损坏")?.clone();
    if !is_dsh_endpoint(&config.dsh_host, config.dsh_port) {
        return Err("DSH 页面尚未就绪，请稍后重试或查看诊断日志".into());
    }
    open_external_url(&format!("http://{}:{}", config.dsh_host, config.dsh_port))
}

fn dsh_window_url(config: &StoredConfig) -> String {
    format!("http://{}:{}", config.dsh_host, config.dsh_port)
}

/// Opens (or focuses) the embedded DSH desktop window. The window is a
/// WebView2 view of the DSH web UI, so DSH no longer needs a browser tab.
fn open_dsh_window(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let config = state.config.lock().map_err(|_| "配置状态锁已损坏")?.clone();
    let url_string = dsh_window_url(&config);
    // 端点检查带重试：DSH 冷启动/慢启动时首查可能未就绪（Starting 状态
    // 最多再等 15 秒），避免"明明在启动却报未运行"。
    let endpoint_ready = is_dsh_endpoint(&config.dsh_host, config.dsh_port);
    if !endpoint_ready {
        let dsh_state = state.dsh.lock().map_err(|_| "DSH 状态锁已损坏")?.state;
        let mut waited = Duration::ZERO;
        while matches!(dsh_state, ServiceState::Starting) && waited < Duration::from_secs(15) {
            thread::sleep(Duration::from_millis(500));
            waited += Duration::from_millis(500);
            if is_dsh_endpoint(&config.dsh_host, config.dsh_port) {
                break;
            }
        }
    }
    if !is_dsh_endpoint(&config.dsh_host, config.dsh_port) {
        // 区分“启动中”与“未运行”，给出明确提示而不是静默失败。
        let dsh_state = state.dsh.lock().map_err(|_| "DSH 状态锁已损坏")?.state;
        let message = if matches!(dsh_state, ServiceState::Starting) {
            "DSH 正在启动，请稍候再打开".to_string()
        } else {
            "DSH 未运行，请先点击“启动 DSH”".to_string()
        };
        return Err(message);
    }
    if let Some(existing) = app.get_webview_window(DSH_WINDOW_LABEL) {
        // 复用已有窗口：恢复显示并导航刷新。销毁后重建 WebView2 曾出现
        // 白屏卡死，因此窗口关闭只隐藏、不销毁（见 on_window_event）。
        // 不吞错误：任何一步失败都给用户明确提示。
        existing
            .show()
            .map_err(|error| format!("无法显示 DSH 窗口：{error}"))?;
        existing
            .set_focus()
            .map_err(|error| format!("无法聚焦 DSH 窗口：{error}"))?;
        let navigate_url = url_string
            .parse::<tauri::Url>()
            .map_err(|error| format!("无效的 DSH 地址：{error}"))?;
        existing
            .navigate(navigate_url)
            .map_err(|error| format!("无法刷新 DSH 窗口：{error}"))?;
        return Ok(());
    }
    let url = url_string
        .parse::<tauri::Url>()
        .map_err(|error| format!("无效的 DSH 地址：{error}"))?;
    let window = WebviewWindowBuilder::new(app, DSH_WINDOW_LABEL, WebviewUrl::External(url))
        .title("DeepSeek Harness")
        .inner_size(1360.0, 900.0)
        .min_inner_size(960.0, 640.0)
        .center()
        // 只允许 DSH 本机页面；其余导航一律拦截。
        .on_navigation(|url| {
            matches!(url.scheme(), "http" | "https")
                && matches!(url.host_str(), Some("127.0.0.1") | Some("localhost"))
        })
        // DSH 页面里的 target=_blank / window.open 交给系统浏览器。
        .on_new_window(|url, _| {
            let _ = open_external_url(url.as_str());
            NewWindowResponse::Deny
        })
        .build()
        .map_err(|error| format!("无法打开 DSH 桌面窗口：{error}"))?;
    let _ = window.set_focus();
    Ok(())
}

#[tauri::command]
fn open_dsh_window_command(app: tauri::AppHandle) -> Result<(), String> {
    open_dsh_window(&app)
}

/// Closes the embedded DSH window once DSH is no longer running, so the
/// window never shows a dead page. Called from status polling commands.
fn sync_dsh_window(app: &tauri::AppHandle, dsh_state: ServiceState) {
    if matches!(dsh_state, ServiceState::Stopped | ServiceState::Error) {
        if let Some(window) = app.get_webview_window(DSH_WINDOW_LABEL) {
            // 隐藏而不是销毁：销毁后再重建 WebView2 窗口曾出现白屏卡死；
            // 隐藏前先卸载页面（about:blank），让 WebView2 释放渲染上下文，
            // 下次 show + navigate 时以干净状态重新加载，降低白屏概率。
            if let Ok(url) = "about:blank".parse::<tauri::Url>() {
                let _ = window.navigate(url);
            }
            let _ = window.hide();
        }
    }
}

fn tail(path: &Path, max_bytes: usize) -> String {
    let Ok(mut file) = File::open(path) else {
        return "(日志尚未生成)".into();
    };
    let mut bytes = Vec::new();
    if file.read_to_end(&mut bytes).is_err() {
        return "(日志读取失败)".into();
    }
    let start = bytes.len().saturating_sub(max_bytes);
    String::from_utf8_lossy(&bytes[start..]).into_owned()
}

#[tauri::command]
fn read_logs(state: State<'_, AppState>) -> String {
    let root = state.runtime.data_root.join("logs");
    format!(
        "=== DSH ===\n{}\n\n=== MCA ===\n{}",
        tail(&root.join("dsh.log"), 80_000),
        tail(&root.join("mca.log"), 50_000)
    )
}

/// 把字符串转成以 NUL 结尾的 UTF-16（Windows API 用）。
#[cfg(target_os = "windows")]
fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

/// 读取 Windows 系统代理（IE/系统设置），返回可注入子进程的代理环境变量。
/// - 已显式设置 `HTTP_PROXY`/`HTTPS_PROXY` 环境变量时不覆盖；
/// - `NO_PROXY` 固定豁免本机回环地址，避免 MCA 内部 127.0.0.1 通信被代理劫持；
/// - MCA 的 httpx 请求与 yt-dlp 子进程都会读取这些环境变量，从而跟随系统代理。
fn system_proxy_env() -> Vec<(String, String)> {
    let mut vars = Vec::new();
    if std::env::var_os("HTTP_PROXY").is_some() || std::env::var_os("HTTPS_PROXY").is_some() {
        return vars;
    }
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::System::Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_READ, RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
        };
        const SETTINGS: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";
        let key = to_wide(SETTINGS);
        let mut hkey: HKEY = std::ptr::null_mut();
        unsafe {
            if RegOpenKeyExW(HKEY_CURRENT_USER, key.as_ptr(), 0, KEY_READ, &mut hkey) != 0 {
                return vars;
            }
            let mut enabled: u32 = 0;
            let mut enabled_size = std::mem::size_of::<u32>() as u32;
            let enabled_name = to_wide("ProxyEnable");
            let mut proxy = vec![0u16; 4096];
            let mut proxy_size = (proxy.len() * 2) as u32;
            let proxy_name = to_wide("ProxyServer");
            let enabled_ok = RegQueryValueExW(
                hkey,
                enabled_name.as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                (&mut enabled as *mut u32) as *mut u8,
                &mut enabled_size,
            ) == 0;
            let proxy_ok = RegQueryValueExW(
                hkey,
                proxy_name.as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                proxy.as_mut_ptr() as *mut u8,
                &mut proxy_size,
            ) == 0;
            RegCloseKey(hkey);
            if !enabled_ok || enabled != 1 || !proxy_ok {
                return vars;
            }
            let server = String::from_utf16_lossy(&proxy[..proxy_size as usize / 2])
                .trim_end_matches('\0')
                .to_string();
            if server.is_empty() {
                return vars;
            }
            // 支持 "http=host:port;https=host:port" 与裸 "host:port" 两种格式。
            let mut http_proxy = String::new();
            let mut https_proxy = String::new();
            for part in server.split(';') {
                if let Some((scheme, address)) = part.split_once('=') {
                    let address = address.trim();
                    if address.is_empty() {
                        continue;
                    }
                    let url = if address.contains("://") {
                        address.to_string()
                    } else {
                        format!("http://{address}")
                    };
                    match scheme.trim() {
                        "http" => http_proxy = url,
                        "https" => https_proxy = url,
                        _ => {}
                    }
                } else if !http_proxy.is_empty() && !https_proxy.is_empty() {
                    break;
                } else {
                    let url = if server.contains("://") {
                        server.clone()
                    } else {
                        format!("http://{server}")
                    };
                    http_proxy = url.clone();
                    https_proxy = url;
                    break;
                }
            }
            if !http_proxy.is_empty() {
                vars.push(("HTTP_PROXY".into(), http_proxy));
            }
            if !https_proxy.is_empty() {
                vars.push(("HTTPS_PROXY".into(), https_proxy));
            }
            if !vars.is_empty() {
                vars.push(("NO_PROXY".into(), "localhost,127.0.0.1,::1".into()));
            }
        }
    }
    vars
}

/// 单实例保护：Windows 命名 Mutex（`Local\` 会话作用域）。
/// - 首次启动：创建成功，句柄故意泄漏以保持到进程退出（互斥体随进程
///   退出自动释放，无需显式关闭）。
/// - 重复启动：Mutex 已存在 → 激活旧实例的主窗口并返回 Err（调用方提示
///   后退出），防止多个控制中心互相抢 DSH/MCA 服务。
#[cfg(target_os = "windows")]
fn ensure_single_instance() -> Result<(), String> {
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, GetWindowTextLengthW, SetForegroundWindow, ShowWindow,
        SW_RESTORE,
    };

    const MUTEX_NAME: &str = "Local\\DSHPlusPlus-SingleInstance";
    let wide = to_wide(MUTEX_NAME);
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, wide.as_ptr()) };
    if handle.is_null() {
        return Err("创建单实例互斥体失败".into());
    }
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS as u32 {
        unsafe {
            CloseHandle(handle);
        }
        // 激活已有实例的控制中心窗口（可能在托盘隐藏或最小化）。
        let target = to_wide("DSH++ 控制中心");
        let target_ptr = target.as_ptr();
        unsafe extern "system" fn activate(hwnd: HWND, lparam: LPARAM) -> i32 {
            let target = lparam as *const u16;
            let len = GetWindowTextLengthW(hwnd);
            if len <= 0 {
                return 1;
            }
            let mut buf = vec![0u16; (len + 1) as usize];
            GetWindowTextW(hwnd, buf.as_mut_ptr(), len + 1);
            // 逐字符比较窗口标题与目标标题（UTF-16，target 以 NUL 结尾）。
            let mut i = 0usize;
            loop {
                let t = *target.add(i);
                if t == 0 {
                    if i == len as usize {
                        ShowWindow(hwnd, SW_RESTORE);
                        SetForegroundWindow(hwnd);
                        return 0;
                    }
                    return 1;
                }
                if i >= buf.len() || buf[i] != t {
                    return 1;
                }
                i += 1;
            }
        }
        unsafe {
            EnumWindows(Some(activate), target_ptr as LPARAM);
        }
        return Err("已有 DSH++ 实例正在运行".into());
    }
    // 裸指针句柄不实现 Drop，函数返回后句柄保持打开（互斥体随进程退出
    // 由 OS 自动释放），无需显式持有或关闭。
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn ensure_single_instance() -> Result<(), String> {
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(error) = ensure_single_instance() {
        eprintln!("[dshplusplus] {error}");
        #[cfg(target_os = "windows")]
        unsafe {
            use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONINFORMATION, MB_OK, MessageBoxW};
            let message = to_wide(
                "已有 DSH++ 正在运行，已切换到现有窗口。\n如果看不到窗口，请查看系统托盘。",
            );
            let caption = to_wide("DSH++");
            MessageBoxW(std::ptr::null_mut(), message.as_ptr(), caption.as_ptr(), MB_ICONINFORMATION | MB_OK);
        }
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let runtime = discover_runtime().map_err(std::io::Error::other)?;
            let config = load_config(&runtime);
            let should_start =
                config.auto_start_dsh || std::env::var_os("DSHPLUSPLUS_AUTO_START").is_some();
            let auto_open = config.auto_open_dsh_window;
            let home = effective_dsh_home(&runtime);
            materialize_dsh_config(&runtime, &config, &home).map_err(std::io::Error::other)?;
            app.manage(AppState {
                config: Mutex::new(config),
                dsh: Mutex::new(ManagedChild::stopped("等待启动")),
                mca: Mutex::new(ManagedChild::stopped("等待启动")),
                browser: Mutex::new(ManagedChild::stopped("等待启动")),
                extension_status: Mutex::new(None),
                runtime,
            });

            // 系统托盘：关闭主窗口只是隐藏，DSH/MCA 继续运行；
            // 只有从托盘菜单“退出”才真正退出并清理服务进程树。
            let show_item = MenuItem::with_id(app, "show", "打开控制中心", true, None::<&str>)
                .map_err(std::io::Error::other)?;
            let quit_item = MenuItem::with_id(
                app,
                "quit",
                "退出 DSH++（同时停止服务）",
                true,
                None::<&str>,
            )
            .map_err(std::io::Error::other)?;
            let menu =
                Menu::with_items(app, &[&show_item, &quit_item]).map_err(std::io::Error::other)?;
            let icon = app
                .default_window_icon()
                .cloned()
                .ok_or_else(|| std::io::Error::other("缺少应用图标"))?;
            TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        let state = app.state::<AppState>();
                        unregister_chrome_native_host();
                        if let Ok(mut browser) = state.browser.lock() {
                            browser.stop();
                        }
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)
                .map_err(std::io::Error::other)?;

            if should_start {
                let handle = app.handle().clone();
                thread::spawn(move || {
                    let state = handle.state::<AppState>();
                    let Ok(config) = state.config.lock().map(|value| value.clone()) else {
                        return;
                    };
                    if config.enable_mca {
                        let _ = start_mca(&state, &config);
                    }
                    if config.enable_browser || config.enable_chrome_use {
                        let _ = start_browser(&state, &config);
                    }
                    if start_dsh(&state, &config).is_ok() {
                        // 旧会话的模型选择平移到 deepseek-plus（图片发送适配）。
                        migrate_sessions_to_plus(&config.dsh_host, config.dsh_port);
                        if auto_open {
                            let window_handle = handle.clone();
                            let _ = handle.run_on_main_thread(move || {
                                let _ = open_dsh_window(&window_handle);
                            });
                        }
                    }
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // 主窗口点 X：不退出，隐藏到托盘。退出只走托盘菜单。
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                } else if window.label() == DSH_WINDOW_LABEL {
                    // DSH 内嵌窗口点 X：只隐藏。销毁后重建 WebView2 曾导致
                    // 白屏卡死，隐藏保留可随时 show + navigate 恢复。
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            enable_computer_provider,
            refresh_status,
            save_config,
            start_services,
            stop_services,
            open_dsh,
            open_dsh_window_command,
            install_chrome_extension,
            chrome_extension_status,
            check_for_update,
            apply_updates,
            open_dsh_guide,
            resolve_dsh_cli_path,
            detect_dsh_cli,
            read_logs,
        ])
        .run(tauri::generate_context!())
        .expect("DSHPlusPlus failed to start");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 更新清单解析：新格式（app/plugins/mca）与旧格式（顶层 version/url）兼容。
    #[test]
    fn update_manifest_parses_new_and_legacy_formats() {
        let new = r#"{
          "app": { "version": "0.1.0-dev.2", "url": "https://example.com/DSHPlusPlus.update.exe" },
          "plugins": { "urlPrefix": "https://example.com/dl/", "packages": { "multimodal": "0.1.0-dev.2", "bundle-plus": "0.1.0-dev.2" } },
          "mca": { "version": "1.0.0", "url": "https://example.com/mca-runtime.exe" }
        }"#;
        let manifest: UpdateManifest = serde_json::from_str(new).expect("新格式应可解析");
        assert_eq!(manifest.app.as_ref().unwrap().version, "0.1.0-dev.2");
        let plugins = manifest.plugins.as_ref().unwrap();
        assert_eq!(plugins.url_prefix, "https://example.com/dl/");
        assert_eq!(plugins.packages.get("multimodal").unwrap(), "0.1.0-dev.2");
        assert_eq!(manifest.mca.as_ref().unwrap().version, "1.0.0");

        let legacy = r#"{ "version": "0.1.0-dev.3", "url": "https://example.com/app.exe" }"#;
        let manifest: UpdateManifest = serde_json::from_str(legacy).expect("旧格式应可解析");
        assert!(manifest.app.is_none(), "旧格式顶层字段不直接进 app（由兼容逻辑合并）");
    }

    /// 创建唯一临时目录（不依赖 tempfile crate），返回后由 drop 清理。
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "dshpp-test-{tag}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&path).unwrap();
            TempDir(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    /// 构造一个 profile 的 Secure Preferences：extensions.settings 下
    /// 写入 chromeUse 扩展记录。
    fn write_extension_record(profile_dir: &Path, record: serde_json::Value) {
        let prefs = json!({
            "extensions": { "settings": { EXTENSION_ID: record } }
        });
        write(
            &profile_dir.join("Secure Preferences"),
            &prefs.to_string(),
        );
    }

    /// 扩展四态判定：无记录 / 健康记录 / path 指向消失目录（残影）/
    /// state:0（禁用）。
    #[test]
    fn extension_status_classifies_four_states() {
        let tmp = TempDir::new("ext-status");
        let user_data = tmp.path().join("User Data");
        let default = user_data.join("Default");
        fs::create_dir_all(&default).unwrap();
        // 当前数据根扩展（期望 manifest：version + key）
        let ext = tmp.path().join("browser-extension");
        write(&ext.join("manifest.json"), r#"{"version":"0.1.0","key":"KEY"}"#);
        let expected: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(ext.join("manifest.json")).unwrap(),
        )
        .unwrap();

        // 未安装：profile 里没有记录
        write(&default.join("Secure Preferences"), r#"{"extensions":{"settings":{}}}"#);
        let (status, profile, path) = scan_browser_extension(&user_data, Some(&expected));
        assert_eq!(status, ExtensionInstallState::NotInstalled);
        assert!(profile.is_none() && path.is_none());

        // 已安装：记录 path 指向真实目录且 manifest 一致
        write_extension_record(
            &default,
            json!({ "state": 1, "path": path_string(&ext) }),
        );
        let (status, profile, path) = scan_browser_extension(&user_data, Some(&expected));
        assert_eq!(status, ExtensionInstallState::Installed);
        assert_eq!(profile.as_deref(), Some("Default"));
        assert_eq!(path.as_deref(), Some(path_string(&ext).as_str()));

        // 失效：path 指向已消失的目录（旧版本残留）
        write_extension_record(
            &default,
            json!({ "state": 1, "path": path_string(&tmp.path().join("gone")) }),
        );
        let (status, _, _) = scan_browser_extension(&user_data, Some(&expected));
        assert_eq!(status, ExtensionInstallState::Stale);

        // 禁用：state:0
        write_extension_record(
            &default,
            json!({ "state": 0, "path": path_string(&ext) }),
        );
        let (status, _, _) = scan_browser_extension(&user_data, Some(&expected));
        assert_eq!(status, ExtensionInstallState::Disabled);
    }

    /// 路径校验：记录目录存在但 manifest version 与当前数据根不一致 ->
    /// 失效（提示一键修复）。
    #[test]
    fn extension_status_marks_version_mismatch_stale() {
        let tmp = TempDir::new("ext-version");
        let user_data = tmp.path().join("User Data");
        let default = user_data.join("Default");
        fs::create_dir_all(&default).unwrap();
        let expected: serde_json::Value =
            serde_json::from_str(r#"{"version":"0.2.0","key":"KEY"}"#).unwrap();
        // 旧版本扩展目录：manifest 存在但 version 不同
        let old = tmp.path().join("old-extension");
        write(&old.join("manifest.json"), r#"{"version":"0.1.0","key":"KEY"}"#);
        write_extension_record(&default, json!({ "state": 1, "path": path_string(&old) }));

        let (status, _, _) = scan_browser_extension(&user_data, Some(&expected));
        assert_eq!(status, ExtensionInstallState::Stale, "版本不一致应判失效");
    }

    /// 多 profile 归并：Default 是残影记录、Profile 1 健康 ->
    /// 浏览器级状态取健康记录（installed > disabled > stale）。
    #[test]
    fn extension_scan_merges_profiles_preferring_healthy_record() {
        let tmp = TempDir::new("ext-merge");
        let user_data = tmp.path().join("User Data");
        let default = user_data.join("Default");
        let profile1 = user_data.join("Profile 1");
        fs::create_dir_all(&default).unwrap();
        fs::create_dir_all(&profile1).unwrap();
        let ext = tmp.path().join("browser-extension");
        write(&ext.join("manifest.json"), r#"{"version":"0.1.0","key":"KEY"}"#);
        let expected: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(ext.join("manifest.json")).unwrap(),
        )
        .unwrap();
        // Default：残影（path 已消失）
        write_extension_record(
            &default,
            json!({ "state": 1, "path": path_string(&tmp.path().join("gone")) }),
        );
        // Profile 1：健康
        write_extension_record(
            &profile1,
            json!({ "state": 1, "path": path_string(&ext) }),
        );

        let (status, profile, _) = scan_browser_extension(&user_data, Some(&expected));
        assert_eq!(status, ExtensionInstallState::Installed);
        assert_eq!(profile.as_deref(), Some("Profile 1"), "应取健康记录所在 profile");
    }

    /// agent shim 内容：包含默认 profile 注入与 mcp 子命令仿真，
    /// launcher（node+bin.js 或独立 exe）原样嵌入。
    #[test]
    fn agent_shim_content_contains_profile_injection_and_mcp_emulation() {
        let content = agent_shim_content(r#""C:\node.exe" "E:\DSH\bin.js""#);
        assert!(content.contains(r#""C:\node.exe" "E:\DSH\bin.js""#), "launcher 原样嵌入");
        assert!(
            content.contains("--profile dshplusplus"),
            "无 profile 时注入默认 profile"
        );
        assert!(content.contains("if \"%1\"==\"mcp\""), "拦截 mcp 子命令");
        assert!(
            content.contains("mca-control-center"),
            "mcp list 读取注册标记"
        );
        assert!(
            content.contains("No MCP servers configured."),
            "无注册时的 list 输出"
        );
    }

    #[test]
    fn resolves_dsh_cli_from_source_and_installed_directories() {
        let tmp = TempDir::new("dsh-cli-resolve");
        let source = tmp.path().join("DeepseekHarness");
        let source_cli = source.join("apps/cli/lib/bin.js");
        write(&source_cli, "// source cli");
        assert_eq!(resolve_dsh_cli_candidate(&source), Some(source_cli.clone()));
        assert_eq!(
            resolve_dsh_cli_candidate(&source.join("apps/cli")),
            Some(source_cli.clone())
        );
        assert_eq!(
            resolve_dsh_cli_candidate(&source.join("apps/cli/lib")),
            Some(source_cli)
        );

        let npm_root = tmp.path().join("node_modules");
        let installed_cli = npm_root.join("@deepseek-ai/dsh/lib/bin.js");
        write(&installed_cli, "// installed cli");
        assert_eq!(resolve_dsh_cli_candidate(&npm_root), Some(installed_cli));
        assert!(resolve_dsh_cli_candidate(&tmp.path().join("missing")).is_none());
    }

    #[test]
    fn finds_sibling_dsh_source_checkout_from_portable_stage() {
        let tmp = TempDir::new("dsh-cli-nearby");
        let stage = tmp.path().join("DSHPlusPlus/zip-stage");
        fs::create_dir_all(&stage).unwrap();
        let cli = tmp.path().join("DeepseekHarness/apps/cli/lib/bin.js");
        write(&cli, "// source cli");

        assert_eq!(find_dsh_near_paths(&[stage]), Some(cli));
    }

    #[test]
    fn merge_workspace_registry_adds_missing_records_by_path() {
        let tmp = TempDir::new("ws-merge");
        let legacy = tmp.path().join("legacy/workspace.json");
        let target = tmp.path().join("target/workspace.json");
        // 旧 home：两个 workspace（E:\DeepSeekPlusPlus 与 D:\DeepSeekHarness）
        write(
            &legacy,
            r#"{
  "unit": { "name": "workspace", "version": 2 },
  "global": { "initialized": true, "workspaceIds": ["bb729f88-f841-48dd-a74e-8a8de3430ac4", "89b555cb-33e4-4342-8f11-c6ab1ea639c3"], "archivedSessionIds": [] },
  "tables": { "workspaces": {
    "bb729f88-f841-48dd-a74e-8a8de3430ac4": { "path": "D:\\SampleProject", "title": "SampleProject", "sessionIds": ["session-4e3dcb3e-ed3c-4be7-ac29-6c69e02b8b29"], "createdAt": "2026-08-14T12:47:40.530Z", "updatedAt": "2026-08-14T16:23:42.035Z" },
    "89b555cb-33e4-4342-8f11-c6ab1ea639c3": { "path": "E:\\SampleProject", "title": "SampleProject", "sessionIds": ["session-f1b0ccdc-59a6-4215-abc5-281785c7c926"], "createdAt": "2026-08-16T07:19:14.481Z", "updatedAt": "2026-08-16T07:29:45.577Z" }
  } }
}"#,
        );
        // 目标 home：已有 D:\DeepSeekHarness（同 path，不同 id）
        write(
            &target,
            r#"{
  "unit": { "name": "workspace", "version": 2 },
  "global": { "initialized": true, "workspaceIds": ["02b0fd35-b0f2-429e-b996-8f5bef696fc8"], "archivedSessionIds": [] },
  "tables": { "workspaces": {
    "02b0fd35-b0f2-429e-b996-8f5bef696fc8": { "path": "D:\\SampleProject", "title": "SampleProject", "sessionIds": ["session-4e3dcb3e-ed3c-4be7-ac29-6c69e02b8b29", "session-2875f894-d63c-4fc6-ad26-b8ed014201bb"], "createdAt": "2026-08-14T12:47:40.530Z", "updatedAt": "2026-08-14T16:23:42.035Z" }
  } }
}"#,
        );
        merge_workspace_registry(&legacy, &target).unwrap();

        let parsed: serde_json::Value =
            serde_json::from_slice(&fs::read(&target).unwrap()).unwrap();
        let ws = parsed["tables"]["workspaces"].as_object().unwrap();
        // 只并入 E:\DeepSeekPlusPlus（D:\ 已有同 path 记录，不重复）
        assert_eq!(ws.len(), 2, "应合并缺失的 workspace 记录");
        assert!(ws.contains_key("89b555cb-33e4-4342-8f11-c6ab1ea639c3"));
        assert!(ws.contains_key("02b0fd35-b0f2-429e-b996-8f5bef696fc8"));
        let ids = parsed["global"]["workspaceIds"].as_array().unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0], "02b0fd35-b0f2-429e-b996-8f5bef696fc8", "原有顺序保持在前");
        assert_eq!(ids[1], "89b555cb-33e4-4342-8f11-c6ab1ea639c3");

        // 幂等：再跑一次不改变内容
        let before = fs::read(&target).unwrap();
        merge_workspace_registry(&legacy, &target).unwrap();
        assert_eq!(before, fs::read(&target).unwrap(), "重复合并必须无副作用");
    }

    #[test]
    fn merge_workspace_registry_copies_whole_file_when_target_missing() {
        let tmp = TempDir::new("ws-copy");
        let legacy = tmp.path().join("legacy/workspace.json");
        let target = tmp.path().join("target/storages/workspace.json");
        write(&legacy, r#"{"unit":{"name":"workspace","version":2},"global":{"initialized":true,"workspaceIds":["a"],"archivedSessionIds":[]},"tables":{"workspaces":{"a":{"path":"D:\\X","title":"X","sessionIds":[],"createdAt":"2026-08-14T00:00:00.000Z","updatedAt":"2026-08-14T00:00:00.000Z"}}}}"#);
        merge_workspace_registry(&legacy, &target).unwrap();
        assert!(target.is_file(), "目标注册表缺失时应整体复制");
        let parsed: serde_json::Value =
            serde_json::from_slice(&fs::read(&target).unwrap()).unwrap();
        assert_eq!(parsed["tables"]["workspaces"]["a"]["path"], "D:\\X");
    }

    #[test]
    fn migrate_portable_home_data_copies_sessions_and_settings_once() {
        let tmp = TempDir::new("migrate");
        let legacy = tmp.path().join("legacy/dsh-home");
        let standard = tmp.path().join("standard/.dsh");
        // 用 USERPROFILE 指向假标准 home（edition 2021 下 set_var 安全；
        // 本测试是唯一读写该变量的用例，避免并行竞争）。
        std::env::set_var("USERPROFILE", tmp.path().join("standard"));
        let runtime = RuntimePaths {
            portable: true,
            data_root: tmp.path().join("legacy"),
            dsh_home: None,
            node: None,
            dsh_cli: None,
            plugins_dir: None,
            mca: None,
            browser_gateway: None,
        };
        // 旧便携 home：2 个会话 + workspace 注册 + settings
        write(
            &legacy.join("sessions/--D-DeepSeekHarness--/session-abc/session.jsonl.zstd"),
            "fake-zstd-1",
        );
        write(
            &legacy.join("sessions/--E-DeepSeekPlusPlus--/session-def/session.jsonl.zstd"),
            "fake-zstd-2",
        );
        write(
            &legacy.join("storages/workspace.json"),
            r#"{"unit":{"name":"workspace","version":2},"global":{"initialized":true,"workspaceIds":["w1"],"archivedSessionIds":[]},"tables":{"workspaces":{"w1":{"path":"E:\\DeepSeekPlusPlus","title":"DeepSeekPlusPlus","sessionIds":["session-def"],"createdAt":"2026-08-16T00:00:00.000Z","updatedAt":"2026-08-16T00:00:00.000Z"}}}}"#,
        );
        write(&legacy.join("settings.yaml"), "agent-default-model:\n  provider: deepseek-official\n");
        // 标准 home 已有 1 个会话（模拟用户旧数据）
        write(
            &standard.join("sessions/--D-DeepSeekHarness--/session-abc/session.jsonl.zstd"),
            "user-original",
        );

        migrate_portable_home_data(&runtime).unwrap();

        // 会话：只复制缺失的 session-def；session-abc 保留用户原文件
        assert_eq!(
            fs::read_to_string(
                standard.join("sessions/--D-DeepSeekHarness--/session-abc/session.jsonl.zstd")
            )
            .unwrap(),
            "user-original",
            "已存在的会话不得被覆盖"
        );
        assert_eq!(
            fs::read_to_string(
                standard.join("sessions/--E-DeepSeekPlusPlus--/session-def/session.jsonl.zstd")
            )
            .unwrap(),
            "fake-zstd-2",
            "缺失的会话应被复制"
        );
        // workspace 注册表：整体复制（目标缺失）
        assert!(standard.join("storages/workspace.json").is_file());
        // settings.yaml：目标缺失时继承
        assert_eq!(
            fs::read_to_string(standard.join("settings.yaml")).unwrap(),
            "agent-default-model:\n  provider: deepseek-official\n"
        );

        // 幂等：再跑一次，用户文件仍不被覆盖，也没有重复复制
        migrate_portable_home_data(&runtime).unwrap();
        assert_eq!(
            fs::read_to_string(
                standard.join("sessions/--D-DeepSeekHarness--/session-abc/session.jsonl.zstd")
            )
            .unwrap(),
            "user-original"
        );
    }

    #[test]
    fn migrate_skipped_when_dsh_home_explicit() {
        let tmp = TempDir::new("migrate-skip");
        let legacy = tmp.path().join("legacy/dsh-home");
        write(
            &legacy.join("sessions/--D-X--/session-abc/session.jsonl.zstd"),
            "fake",
        );
        let runtime = RuntimePaths {
            portable: true,
            data_root: tmp.path().join("legacy"),
            dsh_home: Some(tmp.path().join("explicit-home")),
            node: None,
            dsh_cli: None,
            plugins_dir: None,
            mca: None,
            browser_gateway: None,
        };
        migrate_portable_home_data(&runtime).unwrap();
        assert!(
            !tmp.path().join("explicit-home").exists(),
            "显式指定 home 时不得迁移"
        );
    }

    /// 构造一个最小的可运行 materialize 环境：假的 dsh_cli（带
    /// node_modules/@dshplusplus 插件包）＋临时 home。
    fn materialize_env(tag: &str) -> (TempDir, RuntimePaths, PathBuf) {
        let tmp = TempDir::new(tag);
        let scope = tmp.path().join("runtime/node_modules/@dshplusplus");
        for pkg in ["multimodal", "multimodal-llm", "multimodal-router", "tool-media-inspect", "bundle-plus"] {
            write(&scope.join(pkg).join("package.json"), r#"{"name":"x","version":"0.0.0"}"#);
        }
        let cli = tmp
            .path()
            .join("runtime/node_modules/@deepseek-ai/dsh/lib/bin.js");
        write(&cli, "// fake cli");
        let home = tmp.path().join("home");
        let runtime = RuntimePaths {
            portable: true,
            data_root: tmp.path().join("data"),
            dsh_home: Some(home.clone()),
            node: None,
            dsh_cli: Some(cli),
            plugins_dir: None,
            mca: None,
            browser_gateway: None,
        };
        (tmp, runtime, home)
    }

    #[test]
    fn materialize_generates_deepseek_plus_from_builtin_defaults() {
        let (_tmp, runtime, home) = materialize_env("mat-defaults");
        let config = StoredConfig::default();
        materialize_dsh_config(&runtime, &config, &home).unwrap();

        let settings_path = home.join("settings.yaml");
        let settings: serde_yaml::Value =
            serde_yaml::from_slice(&fs::read(&settings_path).unwrap()).unwrap();
        let plus = &settings["llm-pi-ai"]["providers"]["deepseek-plus"];
        assert_eq!(plus["apiKeyEnv"], "DEEPSEEK_API_KEY");
        assert_eq!(plus["baseURL"], "https://api.deepseek.com");
        let models = plus["models"].as_sequence().unwrap();
        assert_eq!(models[0]["id"], "deepseek-v4-flash");
        assert_eq!(models[0]["contextWindow"], 1_000_000u64);
        assert_eq!(models[0]["maxTokens"], 256_000u64);
        let input = models[0]["input"].as_sequence().unwrap();
        assert!(
            input.iter().any(|v| v == "image"),
            "deepseek-plus 模型必须声明 image 输入"
        );
        // 新用户没有 agent-default-model 段：应创建指向 deepseek-plus 的默认段
        assert_eq!(settings["agent-default-model"]["provider"], "deepseek-plus");
        assert_eq!(settings["agent-default-model"]["model"], "deepseek-v4-flash");
    }

    #[test]
    fn materialize_preserves_existing_primary_config() {
        let (_tmp, runtime, home) = materialize_env("mat-existing");
        let mut config = StoredConfig::default();
        // 用户显式配置过 llm-deepseek（自定义 baseURL 与模型）
        write(
            &home.join("settings.yaml"),
            r#"agent-default-model:
  provider: deepseek-official
  model: deepseek-v4-pro
  reasoningEffort: max
llm-deepseek:
  apiKeyEnv: MY_DEEPSEEK_KEY
  baseURL: https://proxy.example.com/v1
  models:
    - id: deepseek-v4-pro
      name: DeepSeek-V4-Pro
      contextWindow: 1000000
      maxTokens: 256000
"#,
        );
        materialize_dsh_config(&runtime, &config, &home).unwrap();

        let settings: serde_yaml::Value =
            serde_yaml::from_slice(&fs::read(home.join("settings.yaml")).unwrap()).unwrap();
        let plus = &settings["llm-pi-ai"]["providers"]["deepseek-plus"];
        // 平移必须沿用用户显式配置的 baseURL 与 apiKeyEnv，而不是默认值
        assert_eq!(plus["baseURL"], "https://proxy.example.com/v1");
        assert_eq!(plus["apiKeyEnv"], "MY_DEEPSEEK_KEY");
        // 默认模型保留用户选择的模型名，仅平移 provider
        assert_eq!(settings["agent-default-model"]["provider"], "deepseek-plus");
        assert_eq!(settings["agent-default-model"]["model"], "deepseek-v4-pro");
        assert_eq!(settings["agent-default-model"]["reasoningEffort"], "max");
    }

    /// 电脑能力（观察/操作）默认开启：开箱即用，安全由 MCA 侧
    /// 风险等级与逐次确认兜底。
    #[test]
    fn computer_capabilities_default_on() {
        let config = StoredConfig::default();
        assert!(config.mca_computer_observe, "观察电脑应默认开启");
        assert!(config.mca_computer_act, "操作电脑应默认开启");
    }

    /// 历史配置中保存的 false 在加载时归一化为 true：
    /// 避免旧配置落进“开关禁用 ↔ Provider 未启用”的死锁。
    #[test]
    fn load_config_forces_computer_capabilities_on() {
        let tmp = TempDir::new("cfg-force-computer");
        let runtime = RuntimePaths {
            portable: true,
            data_root: tmp.path().join("data"),
            dsh_home: None,
            node: None,
            dsh_cli: None,
            plugins_dir: None,
            mca: None,
            browser_gateway: None,
        };
        write(
            &runtime.data_root.join("dshplusplus.json"),
            r#"{"dshPort":18760,"mcaComputerObserve":false,"mcaComputerAct":false}"#,
        );
        let config = load_config(&runtime);
        assert!(config.mca_computer_observe, "加载后观察电脑应被强制开启");
        assert!(config.mca_computer_act, "加载后操作电脑应被强制开启");
    }

    /// MCA 使用 DSH++ 专属端口（18767，紧邻 BROWSER_PORT），并把
    /// agent-base-url 指向自己：与独立 MCA Control Center（8766）互不探测。
    #[test]
    fn mca_serve_args_use_dedicated_port_and_self_agent_base_url() {
        assert_eq!(MCA_PORT, 18767, "MCA 端口必须是 DSH++ 专属端口");
        let args = mca_serve_args(MCA_PORT);
        let flag = |name: &str| {
            args.iter()
                .position(|arg| arg == name)
                .unwrap_or_else(|| panic!("serve 参数缺少 {name}"))
                + 1
        };
        assert_eq!(args[0], "serve");
        assert_eq!(args[flag("--port")], "18767");
        assert_eq!(
            args[flag("--agent-base-url")],
            "http://127.0.0.1:18767",
            "健康探测必须指向自己，而非外部 MCA 的 8766"
        );
    }

    /// enable_desktop_provider 必须向桌面 Provider 状态端点 POST enabled=true：
    /// 这是解开电脑开关死锁的出口（无需依赖已保存的能力勾选）。
    #[test]
    fn enable_desktop_provider_posts_enabled_true() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // 完整读完请求（头 + Content-Length 指定的体）再应答：
            // 残留未读数据会在连接关闭时触发 RST，中断客户端读响应。
            let mut buffer = Vec::new();
            let mut chunk = [0u8; 1024];
            loop {
                let read = stream.read(&mut chunk).unwrap();
                if read == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..read]);
                if let Some(header_end) = find_subsequence(&buffer, b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
                    let content_length = headers
                        .lines()
                        .find_map(|line| line.split_once(':').and_then(|(name, value)| {
                            (name.trim().eq_ignore_ascii_case("content-length"))
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        }))
                        .unwrap_or(0);
                    if buffer.len() >= header_end + 4 + content_length {
                        break;
                    }
                }
            }
            let request = String::from_utf8_lossy(&buffer).into_owned();
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                )
                .unwrap();
            request
        });
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(5)))
            .build()
            .into();
        enable_desktop_provider(&agent, port).expect("启用请求应成功");
        let request = server.join().unwrap();
        assert!(
            request.starts_with("POST /api/providers/wheel.pyautogui-desktop/state"),
            "请求行应命中桌面 Provider 状态端点，实际：{}",
            request.lines().next().unwrap_or_default()
        );
        let body = request
            .split("\r\n\r\n")
            .nth(1)
            .unwrap_or_default();
        let payload: serde_json::Value = serde_json::from_str(body)
            .unwrap_or_else(|error| panic!("请求体应为 JSON，实际：{body}（{error}）"));
        assert_eq!(
            payload["enabled"], serde_json::Value::Bool(true),
            "请求体应为 enabled=true"
        );
    }

    /// 构造一个 npm 全局 bin 目录的常见布局：无扩展名 sh shim、dsh.cmd、
    /// dsh.ps1 并存，包本体在 node_modules 下。
    fn write_npm_bin_layout(dir: &Path) {
        write(&dir.join("dsh"), "#!/bin/sh\nnode \"$0\" \n");
        write(&dir.join("dsh.cmd"), "@echo off\r\nnode \"%~dp0\\node_modules\\@deepseek-ai\\dsh\\lib\\bin.js\" %*\r\n");
        write(&dir.join("dsh.ps1"), "node \"$PSScriptRoot\\node_modules\\@deepseek-ai\\dsh\\lib\\bin.js\" $args\r\n");
        write(
            &dir.join("node_modules/@deepseek-ai/dsh/lib/bin.js"),
            "#!/usr/bin/env node\n",
        );
    }

    /// Windows 上无扩展名的 npm sh shim 与 .ps1 无法被 CreateProcess 执行，
    /// 不能作为 CLI 候选（os error 193 的根因）。
    #[cfg(target_os = "windows")]
    #[test]
    fn resolve_dsh_cli_candidate_rejects_windows_unlaunchable_files() {
        let tmp = TempDir::new("cli-reject");
        write_npm_bin_layout(tmp.path());
        assert!(
            resolve_dsh_cli_candidate(&tmp.path().join("dsh")).is_none(),
            "无扩展名 sh shim 应被拒绝"
        );
        assert!(
            resolve_dsh_cli_candidate(&tmp.path().join("dsh.ps1")).is_none(),
            "dsh.ps1 应被拒绝"
        );
        assert!(
            resolve_dsh_cli_candidate(&tmp.path().join("dsh.cmd")).is_some(),
            "dsh.cmd 应被接受"
        );
        assert!(
            resolve_dsh_cli_candidate(&tmp.path().join("node_modules/@deepseek-ai/dsh/lib/bin.js"))
                .is_some(),
            "bin.js 应被接受"
        );
    }

    /// 传入 npm 全局 bin 目录时应越过同目录的 sh shim / .ps1，解析到包本
    /// 体的 bin.js。
    #[test]
    fn resolve_dsh_cli_candidate_prefers_package_bin_in_npm_dir() {
        let tmp = TempDir::new("cli-npm-dir");
        write_npm_bin_layout(tmp.path());
        let cli = resolve_dsh_cli_candidate(tmp.path()).expect("npm 目录应能解析出 CLI");
        assert!(
            cli.ends_with("node_modules/@deepseek-ai/dsh/lib/bin.js")
                || cli.ends_with("node_modules\\@deepseek-ai\\dsh\\lib\\bin.js"),
            "应解析到包本体 bin.js，实际：{}",
            cli.display()
        );
    }
}
