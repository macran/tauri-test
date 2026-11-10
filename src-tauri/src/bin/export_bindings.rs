// 只导出 src/bindings.ts，不启动应用：cargo run --bin export_bindings
fn main() {
    tauri_test_lib::export_bindings();
}
