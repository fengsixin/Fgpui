// 防止 windows 子系统下闪出控制台窗口（release 版）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    fgpui_lib::run()
}
