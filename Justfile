git_revision := `git rev-parse --short HEAD`
app_version := `awk -F'"' '/^\[package\]/{p=1} p && /^version *=/{print $2; exit}' Cargo.toml`
build_date := `date -u +%Y-%m-%dT%H:%M:%SZ`

container_runner := "docker"
container_image := "ghcr.io/takayuki-todo/lsef"

test:
    cargo llvm-cov

update_version version:
    sh .github/scripts/update_version.sh {{version}}

docs:
    cargo llvm-cov --html
    cd docs && {{container_runner}} run -it --rm hugomods/hugo:0.163.0
    rm -rf docs/public/coverage && cp -r target/llvm-cov/html docs/public/coverage

build: test
    cargo build --release

container-local:
    {{container_runner}} build \
        --build-arg GIT_REVISION={{git_revision}} \
        --build-arg BUILD_DATE={{build_date}} \
        --build-arg VERSION={{app_version}} \
        -t {{container_image}}:latest \
        -t {{container_image}}:{{app_version}} \
        -f Containerfile \
        .

container:
    {{container_runner}} buildx build --push \
        --platform linux/amd64,linux/arm64 \
        --build-arg GIT_REVISION={{git_revision}} \
        --build-arg BUILD_DATE={{build_date}} \
        --build-arg VERSION={{app_version}} \
        -t {{container_image}}:latest \
        -t {{container_image}}:{{app_version}} \
        -f Containerfile \
        .
