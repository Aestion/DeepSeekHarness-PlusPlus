// GUI 子系统：避免启动时弹出控制台窗口（Console 子系统会让双击启动
// 带一个标题为 exe 路径的黑色命令窗口，从终端启动还会占用终端）。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    dshplusplus_desktop_lib::run();
}
