use std::env;
use std::io::{self, Write};
use std::process;

/// CLI プロセスの入口として、環境変数と OS 引数をライブラリ層へ渡す。
/// 実際の一覧作成や整形は `lsef::run_from_parts` に任せ、ここでは返却された
/// stdout/stderr/終了コードをプロセスへ反映するだけにしている。
fn main() {
    let colors = env::var("LS_COLORS").ok();
    let result = lsef::run_from_parts(env::args_os(), colors);

    write_text(io::stdout(), &result.stdout);
    write_text(io::stderr(), &result.stderr);
    process::exit(result.code);
}

/// 空でない文字列だけを指定された出力ストリームへ書き込む。
/// 書き込み失敗は終了処理中の補助処理として握りつぶし、ライブラリ層の結果を
/// `println!` に依存せずそのまま流せるようにする。
fn write_text(mut stream: impl Write, text: &str) {
    if !text.is_empty() {
        let _ = stream.write_all(text.as_bytes());
    }
}
