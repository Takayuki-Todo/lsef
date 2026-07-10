use clap::Command;
use clap_complete::Shell;
use std::path::Path;

fn generate_impl(s: Shell, app: &mut Command, appname: &str, outdir: &Path, file: String) {
    let destfile = outdir.join(file);
    std::fs::create_dir_all(destfile.parent().unwrap()).unwrap();
    if let Ok(mut dest) = std::fs::File::create(destfile) {
        clap_complete::generate(s, app, appname, &mut dest);
    }
}
pub(super) fn generate(outdir: &Path) {
    use clap_complete::Shell::{Bash, Elvish, Fish, PowerShell, Zsh};
    let appname = "lsef";
    let mut app = crate::args::completion_command();
    app.set_bin_name(appname);
    generate_impl(Bash, &mut app, appname, outdir, format!("bash/{appname}"));
    generate_impl(
        Elvish,
        &mut app,
        appname,
        outdir,
        format!("elvish/{appname}"),
    );
    generate_impl(Fish, &mut app, appname, outdir, format!("fish/{appname}"));
    generate_impl(
        PowerShell,
        &mut app,
        appname,
        outdir,
        format!("powershell/{appname}"),
    );
    generate_impl(Zsh, &mut app, appname, outdir, format!("zsh/_{appname}"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    /// 対応している各 shell の補完ファイルが、期待する配置と内容で生成されることを確認する。
    #[test]
    fn generate_writes_completion_files_for_each_shell() {
        let dir = TempDir::new("gencomp");

        generate(dir.path());

        for (relative, marker) in [
            ("bash/lsef", "complete -F _lsef"),
            ("elvish/lsef", "edit:completion:arg-completer[lsef]"),
            ("fish/lsef", "complete -c lsef"),
            ("powershell/lsef", "Register-ArgumentCompleter"),
            ("zsh/_lsef", "#compdef lsef"),
        ] {
            let content = fs::read_to_string(dir.path().join(relative)).expect("read completion");
            assert!(content.contains(marker), "{relative} missing {marker}");
            assert!(
                content.contains("completions"),
                "{relative} missing completions option"
            );
            assert!(
                content.contains("version"),
                "{relative} missing version option"
            );
        }
    }

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        /// テストごとに衝突しにくい一時ディレクトリを作る。
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(unique_name(label));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }

        /// 生成先として使う一時ディレクトリのパスを返す。
        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        /// テスト終了時に一時ディレクトリを片付ける。
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

    /// プロセス内の並列テストでも一意になりやすいディレクトリ名を作る。
    fn unique_name(label: &str) -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time after epoch")
            .as_nanos();
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        format!("lsef-{label}-{}-{nanos}-{id}", std::process::id())
    }
}
