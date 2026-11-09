//! LumeLog Tauri v2 entry point. The whole point of this crate is to be THIN:
//! every command forwards to `lumelog-core` (spec §3 — heavy logic lives in the
//! pure-Rust crate, reused by the CLI in Task 7 and this shell). Streams that
//! the front end consumes are delivered over `tauri::ipc::Channel`s.

mod ai;
mod can_export;
mod commands;
mod dto;
mod gitinfo;
mod keymap;
mod launch;
// Native macOS application menu (items forward `menu:` ids to the front end).
mod menu;
mod mf4;
mod plugins;
mod recent;
mod recovery;
mod script;
mod state;
mod viewers;
mod wasmi_host;
mod watch;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    launch::collect();
    tauri::Builder::default()
        .menu(|app| menu::build(app))
        .on_menu_event(|app, event| {
            use tauri::Emitter;
            let id = event.id().0.as_str();
            if id.starts_with("menu:") {
                let _ = app.emit("menu://action", id);
            }
        })
        .setup(|app| {
            // AppKit appends English AutoFill / Dictation / Emoji items to the
            // Edit menu once the menu is installed; strip them after the run
            // loop is up (needs the main thread, so hop back from a timer).
            #[cfg(target_os = "macos")]
            {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(600));
                    let _ = handle.run_on_main_thread(menu::strip_system_edit_items);
                });
            }
            Ok(())
        })
        // Single-instance MUST be the first plugin (its callback fires before
        // any window exists): forward the second launch's file args and focus.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            use tauri::{Emitter, Manager};
            let paths = launch::from_argv(&argv.to_vec());
            if !paths.is_empty() {
                let _ = app.emit("second-instance-open", paths);
            }
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppDocs::default())
        .manage(mf4::Mf4Sessions::default())
        .manage(watch::AppWatch::default())
        .manage(std::sync::Arc::new(std::sync::Mutex::new(
            viewers::ViewerSessions::default(),
        )))
        .invoke_handler(tauri::generate_handler![
            commands::app_ping,
            commands::doc_open,
            commands::doc_chunk,
            commands::doc_reload,
            commands::doc_tail_append,
            commands::doc_set_encoding,
            commands::doc_close,
            commands::watch_subscribe,
            commands::watch_status,
            commands::keymap_get,
            commands::keymap_set,
            commands::keymap_reset,
            ai::ai_config_get,
            ai::ai_config_set,
            ai::ai_chat_stream,
            ai::ai_chat_cancel,
            ai::ai_models_list,
            ai::ai_save_text,
            commands::mf4_open,
            commands::mf4_close,
            commands::mf4_channels,
            commands::mf4_records,
            commands::mf4_series,
            commands::mf4_summary,
            commands::mf4_export_csv,
            commands::mf4_export_cancel,
            commands::script_list,
            commands::script_read,
            commands::script_write,
            commands::script_delete,
            commands::script_run,
            commands::script_cancel,
            plugins::syntax_plugins_list,
            plugins::parser_plugins_list,
            commands::parser_run,
            commands::parser_cancel,
            commands::viewer_open,
            commands::viewer_rows,
            commands::viewer_close,
            commands::viewer_export_csv,
            commands::viewer_pdf_bytes,
            recent::recent_list,
            recent::recent_clear,
            commands::doc_save,
            commands::doc_list,
            commands::search_run,
            commands::search_cancel,
            commands::dbc_import,
            commands::can_parse,
            commands::can_parse_blf,
            commands::can_cancel,
            commands::can_decode_snippet,
            commands::filter_run,
            commands::filter_cancel,
            commands::doc_lines_at,
            commands::export_lines,
            commands::export_cancel,
            commands::export_csv,
            commands::export_jsonl,
            commands::can_export_blf_csv,
            commands::can_export_blf_jsonl,
            commands::export_can_cancel,
            commands::recovery_save,
            commands::recovery_list,
            commands::recovery_load,
            commands::recovery_discard,
            commands::recovery_enabled,
            commands::recovery_set_enabled,
            commands::git_probe,
            commands::git_status,
            commands::git_log,
            commands::git_commit_files,
            commands::git_diff_file,
            commands::git_init,
            commands::git_commit,
            commands::git_commit_all,
            commands::git_stage_path,
            commands::git_unstage_path,
            commands::git_discard_path,
            commands::git_branches,
            commands::git_branch_create,
            commands::git_checkout_branch,
            commands::git_checkout_commit,
            commands::git_revert_commit,
            commands::git_remote_status,
            commands::git_remotes,
            commands::git_remote_add,
            commands::git_remote_set_url,
            commands::git_remote_remove,
            commands::git_fetch,
            commands::git_pull,
            commands::git_push,
            commands::git_merge_state,
            commands::git_merge_commit,
            commands::git_merge_abort,
            commands::git_conflicts,
            commands::git_resolve_conflict,
            commands::fif_run,
            commands::fif_cancel,
            commands::csv_run,
            launch::launch_args,
        ])
        .run(tauri::generate_context!())
        .expect("error while running LogLume");
}
