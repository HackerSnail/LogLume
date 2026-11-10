//! Native macOS application menu. Menu items only carry a `menu:`-prefixed
//! id — `lib.rs` forwards it to the front end over `menu://action` and the web
//! UI runs the matching action, so all behavior stays in `main.ts` and this
//! file only describes the structure (spec: thin IPC shell).

use tauri::menu::{Menu, MenuItem, MenuItemBuilder};
#[cfg(target_os = "macos")]
use tauri::menu::{AboutMetadata, SubmenuBuilder};
use tauri::{AppHandle, Runtime};

/// Menu item with an optional accelerator, bound to a `menu:`-prefixed id.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn mi<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
    label: &str,
    accel: Option<&str>,
) -> tauri::Result<MenuItem<R>> {
    let mut b = MenuItemBuilder::with_id(id, label);
    if let Some(a) = accel {
        b = b.accelerator(a);
    }
    b.build(app)
}

#[cfg(target_os = "macos")]
pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let about = AboutMetadata {
        name: Some("LogLume".into()),
        version: Some(env!("CARGO_PKG_VERSION").into()),
        ..Default::default()
    };

    // App menu (LogLume): About / Services / Hide / Quit. Predefined items
    // follow the system locale by default, so Chinese labels are forced via
    // the `_with_text` variants (no system language change required).
    let app_menu = SubmenuBuilder::new(app, "LogLume")
        .about_with_text("关于 LogLume", Some(about))
        .separator()
        .services_with_text("服务")
        .separator()
        .hide_with_text("隐藏 LogLume")
        .hide_others_with_text("隐藏其他")
        .show_all_with_text("全部显示")
        .separator()
        .quit_with_text("退出 LogLume")
        .build()?;

    // 文件: open / save / close / recents. Accelerators mirror the default
    // keymap (keymap.ts DEFAULT_BINDINGS; macOS consumes menu keys before the
    // webview sees them, so there is no double dispatch).
    let file = SubmenuBuilder::new(app, "文件")
        .item(&mi(app, "menu:open", "打开文件…", Some("CmdOrCtrl+O"))?)
        .item(&mi(app, "menu:open-mf4", "打开 MF4 测量数据…", None)?)
        .item(&mi(app, "menu:open-blf", "打开 BLF 报文记录…", None)?)
        .item(&mi(app, "menu:viewer", "打开文档查看器…", None)?)
        .separator()
        .item(&mi(app, "menu:save", "保存", Some("CmdOrCtrl+S"))?)
        .item(&mi(
            app,
            "menu:save-as",
            "另存为…",
            Some("CmdOrCtrl+Shift+S"),
        )?)
        .separator()
        .item(&mi(app, "menu:close", "关闭当前文档", None)?)
        .separator()
        .item(&mi(app, "menu:recent-clear", "清除最近文件记录", None)?)
        .build()?;

    // 编辑: native clipboard / text-editing items (drive the webview).
    let edit = SubmenuBuilder::new(app, "编辑")
        .undo_with_text("撤销")
        .redo_with_text("重做")
        .separator()
        .cut_with_text("剪切")
        .copy_with_text("复制")
        .paste_with_text("粘贴")
        .select_all_with_text("全选")
        .build()?;

    // 查看: search/jump, font zoom, layout toggles, fullscreen.
    let view = SubmenuBuilder::new(app, "查看")
        .item(&mi(
            app,
            "menu:palette",
            "命令面板…",
            Some("CmdOrCtrl+Shift+P"),
        )?)
        .item(&mi(
            app,
            "menu:query",
            "行过滤 / 高亮命中…",
            Some("CmdOrCtrl+Shift+F"),
        )?)
        .item(&mi(app, "menu:jump", "跳转到行…", Some("CmdOrCtrl+G"))?)
        .separator()
        .item(&mi(app, "menu:replace-preview", "查找并替换（预览）…", None)?)
        .item(&mi(app, "menu:fif", "在文件中查找…", None)?)
        .separator()
        .item(&mi(app, "menu:wrap", "切换自动换行", None)?)
        .item(&mi(app, "menu:font-in", "放大字体", None)?)
        .item(&mi(app, "menu:font-out", "缩小字体", None)?)
        .item(&mi(app, "menu:font-reset", "重置字体", None)?)
        .separator()
        .item(&mi(app, "menu:sidebar", "显示 / 隐藏侧栏", None)?)
        .item(&mi(app, "menu:inspector", "显示 / 隐藏文档信息栏", None)?)
        .separator()
        .fullscreen_with_text("进入全屏")
        .build()?;

    // 数据: CAN dock / DBC / CAN export / parser dock / scripts.
    let data = SubmenuBuilder::new(app, "数据")
        .item(&mi(app, "menu:can-panel", "报文面板：显示 / 隐藏", None)?)
        .item(&mi(app, "menu:can-import", "导入 DBC…", None)?)
        .item(&mi(app, "menu:can-parse", "解析当前文档为报文", None)?)
        .item(&mi(app, "menu:can-snippet", "识别选中为报文", None)?)
        .separator()
        .item(&mi(app, "menu:can-export-csv", "导出报文 CSV…", None)?)
        .item(&mi(app, "menu:can-export-jsonl", "导出报文 JSONL…", None)?)
        .separator()
        .item(&mi(app, "menu:parser-panel", "插件解析面板", None)?)
        .item(&mi(app, "menu:script", "脚本管理器…", None)?)
        .build()?;

    // 设置: themes + keymap UI.
    let settings = SubmenuBuilder::new(app, "设置")
        .item(&mi(app, "menu:theme-light", "主题：浅色", None)?)
        .item(&mi(app, "menu:theme-dark", "主题：暗黑", None)?)
        .item(&mi(app, "menu:theme-vampire", "主题：吸血鬼 (Dracula)", None)?)
        .item(&mi(app, "menu:theme-auto", "主题：跟随系统", None)?)
        .separator()
        .item(&mi(app, "menu:keymap", "快捷键设置…", None)?)
        .build()?;

    // 窗口: native window management.
    let window = SubmenuBuilder::new(app, "窗口")
        .minimize_with_text("最小化")
        .maximize_with_text("缩放")
        .separator()
        .bring_all_to_front_with_text("前置全部窗口")
        .build()?;

    Menu::with_items(
        app,
        &[&app_menu, &file, &edit, &view, &data, &settings, &window],
    )
}

#[cfg(not(target_os = "macos"))]
pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    Menu::new(app)
}

/// macOS injects AutoFill / Start Dictation… / Emoji & Symbols into any menu
/// carrying text-editing items, labelled in the *bundle's* localization (this
/// app ships none, so they came up English next to our Chinese items). Drop
/// them once the menu is installed so the Edit menu stays fully Chinese.
#[cfg(target_os = "macos")]
pub fn strip_system_edit_items() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    let Some(main_menu) = app.mainMenu() else {
        return;
    };
    for i in 0..main_menu.numberOfItems() {
        let Some(top) = main_menu.itemAtIndex(i) else {
            continue;
        };
        let Some(submenu) = top.submenu() else {
            continue;
        };
        // Walk backwards so removing items never shifts the pending indices.
        for j in (0..submenu.numberOfItems()).rev() {
            let Some(item) = submenu.itemAtIndex(j) else {
                continue;
            };
            let action = item
                .action()
                .map(|sel| sel.name().to_string_lossy().into_owned())
                .unwrap_or_default();
            let title = item.title().to_string();
            if action == "startDictation:"
                || action == "orderFrontCharacterPalette:"
                || title == "AutoFill"
                || title.starts_with("Start Dictation")
                || title.starts_with("Emoji")
            {
                #[cfg(debug_assertions)]
                eprintln!("[menu] removed system item: {title}");
                submenu.removeItemAtIndex(j);
            }
        }
    }
}
