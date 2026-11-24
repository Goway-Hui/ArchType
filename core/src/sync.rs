//! 锁中毒时的统一处理。
//!
//! 输入法跑在宿主应用的进程里（fcitx5 插件、macOS 外壳、Windows TSF），
//! 一次 panic 会连累整个输入法界面。这里的锁保护的都是缓存与快照（词频、上文、
//! 模型状态），中毒只说明某个线程在写入时 panic 了，值本身仍然可读；
//! 直接取回内部值比把 `PoisonError` 抛给调用方更安全，也不会让后续按键全部失效。
//!
//! 只集中策略，不隐藏错误：调用点仍然显式写出 `mutex_lock(&x)` / `write_lock(&x)`，
//! 读代码时能看出这里取的是哪把锁。
//!
//! 这是 crate 内部工具，不承诺跨版本稳定。

use std::sync::{Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// 读锁；中毒时取回内部值。
pub fn read_lock<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(PoisonError::into_inner)
}

/// 写锁；中毒时取回内部值。
pub fn write_lock<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write().unwrap_or_else(PoisonError::into_inner)
}

/// 互斥锁；中毒时取回内部值。
pub fn mutex_lock<T>(lock: &Mutex<T>) -> MutexGuard<'_, T> {
    lock.lock().unwrap_or_else(PoisonError::into_inner)
}
