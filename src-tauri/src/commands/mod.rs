//! Tauri 命令模块：汇集前端可调用的所有 invoke 命令。
//!
//! 按功能领域拆分为子模块（发票管理、导入、识别、导出、配置等），
//! 通过 `pub use` 统一导出供 Tauri 注册使用。

mod agent;
mod config;
mod email;
mod event;
mod export;
mod import;
mod invoice;
mod recognize;
mod semantic;
mod util;
mod watcher;
mod window;

pub use agent::*;
pub use config::*;
pub use email::*;
pub use event::*;
pub use export::*;
pub use import::*;
pub use invoice::*;
pub use recognize::*;
pub use semantic::*;
pub use util::*;
pub use watcher::*;
pub use window::*;
