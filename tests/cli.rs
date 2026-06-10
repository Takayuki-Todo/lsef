use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

/// 実ビルドされた `lsef` バイナリを起動し、標準入出力と終了コードを検証可能にする。
/// integration test から実プロセスを通すことで、CLI 層の書き出し処理も確認する。
fn run_lsef(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lsef"))
        .args(args)
        .env_remove("LS_COLORS")
        .output()
        .expect("run lsef binary")
}

/// バイナリの `--help` が成功終了し、利用方法を stdout に出すことを確認する。
#[test]
fn help_exits_successfully() {
    let output = run_lsef(&["--help"]);
    assert!(output.status.success());
    assert!(stdout(&output).contains("lsef [OPTIONS] [PATH ...]"));
    assert!(stderr(&output).is_empty());
}

/// plain 出力を実バイナリ経由で実行し、対象ディレクトリのファイル名が stdout に出ることを確認する。
#[test]
fn plain_output_lists_directory_entries() {
    let dir = TempDir::new("plain-cli");
    dir.file("visible.txt", "visible");
    let output = run_lsef(&["--output", "plain", dir.path_str()]);
    assert!(output.status.success());
    assert_eq!(stdout(&output), "visible.txt\n");
    assert!(stderr(&output).is_empty());
}

/// 存在しないパスを実バイナリに渡したとき、非 0 終了と stderr で失敗を返すことを確認する。
#[test]
fn missing_path_exits_with_error() {
    let output = run_lsef(&["definitely-missing-lsef-cli-path"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("definitely-missing-lsef-cli-path"));
}

/// stdout のバイト列を UTF-8 文字列として取得する。
/// この CLI のテストデータは ASCII だけなので、変換失敗はテスト前提の崩れとして扱う。
fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is utf-8")
}

/// stderr のバイト列を UTF-8 文字列として取得する。
/// エラーメッセージ検証を読みやすくするため、呼び出し側では文字列として扱う。
fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is utf-8")
}

struct TempDir {
    path: PathBuf,
    path_text: String,
}

impl TempDir {
    /// 一意な一時ディレクトリを作り、CLI 引数として渡せる文字列表現も保持する。
    /// `Command::args` に `&str` を渡すテストを簡潔にするため、所有する文字列を構造体に持たせる。
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(unique_name(label));
        fs::create_dir_all(&path).expect("create temp dir");
        let path_text = path_to_string(&path);
        Self { path, path_text }
    }

    /// CLI 引数として渡す一時ディレクトリの文字列表現を返す。
    /// 構造体が所有する文字列への参照なので、テスト内で安全に `&str` として使える。
    fn path_str(&self) -> &str {
        &self.path_text
    }

    /// 一時ディレクトリ配下にテスト用ファイルを作成する。
    /// CLI の一覧対象を最小限にし、出力の期待値を決定的にするために使う。
    fn file(&self, name: &str, text: &str) {
        fs::write(self.path.join(name), text).expect("write temp file");
    }
}

impl Drop for TempDir {
    /// テスト終了時に一時ディレクトリを削除する。
    /// 削除失敗はテスト対象の CLI 仕様ではないため、結果を上書きしないよう無視する。
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// OS パスを CLI 引数用の UTF-8 文字列へ変換する。
/// テストで作るパスは ASCII 構成なので、非 UTF-8 はテスト前提の崩れとして panic させる。
fn path_to_string(path: &Path) -> String {
    path.to_str().expect("path is utf-8").to_string()
}

/// 一時ディレクトリ名に使う衝突しにくい名前を生成する。
/// ラベル、プロセス ID、現在時刻を含め、並列実行でも同名になりにくくする。
fn unique_name(label: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos();
    format!("lsef-{label}-{}-{nanos}", std::process::id())
}
