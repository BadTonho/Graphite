//! File-type icon mapping for the explorer, quick open and editor tabs.
//!
//! Icons come from the embedded `Graphite Icons` subset in `graphite-render`.
//! The mapping is deliberately data-only: extensions and special filenames map
//! to a glyph and a brand/neutral color.

use std::borrow::Cow;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FileIcon {
    pub(crate) glyph: char,
    pub(crate) color: u32,
}

macro_rules! file_icons {
    ($( $name:ident = ($glyph:literal, $color:literal) ),* $(,)?) => {
        $(
            pub(crate) const $name: FileIcon = FileIcon {
                glyph: $glyph,
                color: $color,
            };
        )*

        #[cfg(test)]
        const ALL: &[FileIcon] = &[ $( $name ),* ];
    };
}

file_icons! {
    FOLDER = ('\u{e5ff}', 0xc09553),
    FOLDER_OPEN = ('\u{e5fe}', 0xc09553),
    FILE = ('\u{ea7b}', 0x8a94a6),
    README = ('\u{eda4}', 0x519aba),
    LOCK = ('\u{e672}', 0x8a8a8a),
    LICENSE = ('\u{e60a}', 0x8a8a8a),
    CONFIG = ('\u{e615}', 0xb8a400),
    EDITORCONFIG = ('\u{e652}', 0xb8a400),
    SETTINGS = ('\u{e690}', 0xb8a400),
    PDF = ('\u{e67d}', 0xe53935),
    ARCHIVE = ('\u{e6aa}', 0xd99a1f),
    IMAGE = ('\u{e60d}', 0xa074c4),
    AUDIO = ('\u{e638}', 0xe8a33d),
    VIDEO = ('\u{e69f}', 0xe8a33d),
    FONT = ('\u{e659}', 0xd99a1f),
    TEXT = ('\u{ec5e}', 0x6b7280),
    WORD = ('\u{e6a5}', 0x2b579a),
    EXCEL = ('\u{e6a6}', 0x217346),
    CSV = ('\u{e64a}', 0x217346),
    BINARY = ('\u{eae8}', 0x8a8a8a),

    RUST = ('\u{e7a8}', 0xce422b),
    PYTHON = ('\u{e73c}', 0x3572a5),
    JAVASCRIPT = ('\u{e74e}', 0xc9a227),
    TYPESCRIPT = ('\u{e8ca}', 0x3178c6),
    REACT = ('\u{e625}', 0x149eca),
    HTML = ('\u{e736}', 0xe34c26),
    CSS = ('\u{e749}', 0x2965f1),
    SASS = ('\u{e74b}', 0xc6538c),
    LESS = ('\u{e758}', 0x6d9eeb),
    STYLUS = ('\u{e759}', 0x8fbf3f),
    VUE = ('\u{e8dc}', 0x41b883),
    SVELTE = ('\u{e8b7}', 0xff3e00),
    ASTRO = ('\u{e735}', 0xff5a03),
    ANGULAR = ('\u{e753}', 0xdd0031),
    JSON = ('\u{e80b}', 0xb5a600),
    MARKDOWN = ('\u{e73e}', 0x519aba),
    TOML = ('\u{e6b2}', 0x9c4221),
    YAML = ('\u{e8eb}', 0xcb171e),
    XML = ('\u{e619}', 0xe37933),
    SVG = ('\u{e698}', 0xd99a1f),
    C = ('\u{e771}', 0x3a7bd5),
    CPP = ('\u{e7a3}', 0xd1496f),
    CSHARP = ('\u{e7b2}', 0x178600),
    JAVA = ('\u{e738}', 0xb07219),
    KOTLIN = ('\u{e81b}', 0x7f52ff),
    GO = ('\u{e724}', 0x00add8),
    RUBY = ('\u{e739}', 0xcc342d),
    RAILS = ('\u{e73b}', 0xcc342d),
    PHP = ('\u{e73d}', 0x4f5d95),
    SWIFT = ('\u{e755}', 0xf05138),
    LUA = ('\u{e620}', 0x51a0d5),
    DART = ('\u{e64c}', 0x0175c2),
    SCALA = ('\u{e68e}', 0xc22d40),
    HASKELL = ('\u{e777}', 0x5e5086),
    ELIXIR = ('\u{e7cd}', 0x6e4a7e),
    ERLANG = ('\u{e7b1}', 0xb83998),
    CLOJURE = ('\u{e768}', 0xdb5855),
    JULIA = ('\u{e624}', 0xa270ba),
    ZIG = ('\u{e6a9}', 0xec915c),
    NIM = ('\u{e677}', 0xd4a017),
    PERL = ('\u{e769}', 0x0298c3),
    POWERSHELL = ('\u{e86c}', 0x2563eb),
    SHELL = ('\u{e760}', 0x6a9955),
    WINDOWS = ('\u{e70f}', 0x4d6bfe),
    R = ('\u{e881}', 0x276dc3),
    FSHARP = ('\u{e7a7}', 0x378bba),
    OCAML = ('\u{e84e}', 0xec6813),
    ELM = ('\u{e7ce}', 0x1293a8),
    CRYSTAL = ('\u{e7ac}', 0x8a8a8a),
    HAXE = ('\u{e7fa}', 0xdf7900),
    VALA = ('\u{e8d1}', 0xa56de2),
    FORTRAN = ('\u{e7de}', 0x4d41b1),
    MATLAB = ('\u{e82a}', 0xe16737),
    SOLIDITY = ('\u{e8a6}', 0x8f8f8f),
    GROOVY = ('\u{e775}', 0x4298b8),
    GRAILS = ('\u{e7b3}', 0x4298b8),
    DOTNET = ('\u{e77f}', 0x512bd4),
    ASSEMBLY = ('\u{e637}', 0x8a8a8a),
    DELPHI = ('\u{e904}', 0xe62431),
    LATEX = ('\u{e81f}', 0x008080),
    JUPYTER = ('\u{e80f}', 0xf37626),
    OBJECTIVEC = ('\u{e84d}', 0x438eff),

    SQL = ('\u{e706}', 0x336791),
    MYSQL = ('\u{e704}', 0x4479a1),
    POSTGRESQL = ('\u{e76e}', 0x336791),
    SQLITE = ('\u{e7c4}', 0x5a9bd4),
    MONGODB = ('\u{e7a4}', 0x47a248),
    REDIS = ('\u{e76d}', 0xdc382d),
    GRAPHQL = ('\u{e7f4}', 0xe10098),
    PRISMA = ('\u{e86e}', 0x5a67d8),
    GRPC = ('\u{e7f5}', 0x6d9eeb),
    TERRAFORM = ('\u{e69a}', 0x7b42bc),
    DOCKER = ('\u{e7b0}', 0x2496ed),
    KUBERNETES = ('\u{e81d}', 0x326ce5),
    HELM = ('\u{e7fb}', 0x277a9f),
    NGINX = ('\u{e776}', 0x009639),
    APACHE = ('\u{e72b}', 0xd22128),
    GIT = ('\u{e702}', 0xf14e32),
    NPM = ('\u{e71e}', 0xcb3837),
    NODE = ('\u{e719}', 0x339933),
    DENO = ('\u{e7c0}', 0x2f9e44),
    WASM = ('\u{e8e0}', 0x654ff0),
    CMAKE = ('\u{e794}', 0x3b6fb5),
    MAKE = ('\u{e673}', 0x6d8086),
    GRADLE = ('\u{e7f2}', 0x5c9ead),
    MAVEN = ('\u{e82c}', 0xc71a36),
    BAZEL = ('\u{e8f9}', 0x43a047),
    DJANGO = ('\u{e71d}', 0x44b78b),
    FLASK = ('\u{e7dc}', 0x8a8a8a),
    FASTAPI = ('\u{e7d5}', 0x05998b),
    SPRING = ('\u{e8ac}', 0x6db33f),
    LARAVEL = ('\u{e73f}', 0xff2d20),
    SYMFONY = ('\u{e757}', 0x8a8a8a),
    WORDPRESS = ('\u{e70b}', 0x21759b),
    JQUERY = ('\u{e750}', 0x0769ad),
    BOOTSTRAP = ('\u{e747}', 0x7952b3),
    TAILWIND = ('\u{e8ba}', 0x0284c7),
    POSTCSS = ('\u{e86a}', 0xdd3a0a),
    VITE = ('\u{e8d6}', 0x646cff),
    WEBPACK = ('\u{e8e3}', 0x1c78c0),
    ROLLUP = ('\u{e892}', 0xec4a3f),
    BABEL = ('\u{e75d}', 0xb8a400),
    GULP = ('\u{e763}', 0xcf4647),
    GRUNT = ('\u{e74c}', 0xfba919),
    ESLINT = ('\u{e7d2}', 0x4b32c3),
    PRETTIER = ('\u{e6b4}', 0x56b3b4),
    JEST = ('\u{e807}', 0xc21325),
    MOCHA = ('\u{e832}', 0x8d6748),
    VITEST = ('\u{e8d9}', 0x729b1b),
    CYPRESS = ('\u{e7b9}', 0x4a4a4a),
    PLAYWRIGHT = ('\u{e863}', 0x2ead33),
    STORYBOOK = ('\u{e8b3}', 0xff4785),
    YARN = ('\u{e8ec}', 0x2c8ebb),
    PNPM = ('\u{e865}', 0xf69220),
    BUN = ('\u{e76f}', 0xf472b6),
    GITLAB = ('\u{e7eb}', 0xfc6d26),
    GITHUB_ACTIONS = ('\u{e7e9}', 0x2088ff),
    JENKINS = ('\u{e767}', 0xd33833),
    FIREBASE = ('\u{e787}', 0xd99a00),
    LINUX = ('\u{e712}', 0xc9a227),
    APPLE = ('\u{e711}', 0x8a8a8a),
    ANDROID = ('\u{e70e}', 0x7cb342),
    CHROME = ('\u{e743}', 0x4285f4),
    FIREFOX = ('\u{e745}', 0xff7139),
    NEXTJS = ('\u{e83e}', 0x888888),
    NUXT = ('\u{e931}', 0x00a86b),
    REMIX = ('\u{e93e}', 0x8a8a8a),
    NIXOS = ('\u{e843}', 0x5277c3),
    TAURI = ('\u{e8bb}', 0xc99a00),
    ELECTRON = ('\u{e7cb}', 0x47848f),
    QT = ('\u{e87d}', 0x2f9e44),
    UNITY = ('\u{e721}', 0x888888),
    GODOT = ('\u{e7ee}', 0x478cbf),
    UNREAL = ('\u{e8cd}', 0x8a8a8a),
    PHOTOSHOP = ('\u{e7b8}', 0x31a8ff),
    ILLUSTRATOR = ('\u{e7b4}', 0xff9a00),
    BLENDER = ('\u{e766}', 0xf5792a),
    VSCODE = ('\u{e8da}', 0x007acc),
}

pub(crate) const CHEVRON_RIGHT: char = '\u{eab6}';
pub(crate) const CHEVRON_DOWN: char = '\u{eab4}';

/// Returns the icon for a directory or file row.
pub(crate) fn for_entry(name: &str, is_dir: bool, expanded: bool) -> FileIcon {
    if is_dir {
        if expanded {
            FOLDER_OPEN
        } else {
            FOLDER
        }
    } else {
        for_name(name)
    }
}

/// Returns the icon for a file name, falling back to a generic file icon.
pub(crate) fn for_name(name: &str) -> FileIcon {
    let lower = lowercase(name);
    let lower = lower.as_ref();

    if let Some(icon) = special_name(lower) {
        return icon;
    }
    if let Some(icon) = special_prefix(lower) {
        return icon;
    }

    let extension = lower
        .rsplit_once('.')
        .map(|(_, extension)| extension)
        .unwrap_or_default();
    extension_icon(extension).unwrap_or(FILE)
}

/// Returns the icon for a filesystem path.
pub(crate) fn for_path(path: &std::path::Path) -> FileIcon {
    path.file_name()
        .map(|name| for_name(&name.to_string_lossy()))
        .unwrap_or(FILE)
}

fn lowercase(name: &str) -> Cow<'_, str> {
    if name.bytes().any(|byte| byte.is_ascii_uppercase()) {
        Cow::Owned(name.to_ascii_lowercase())
    } else {
        Cow::Borrowed(name)
    }
}

fn special_name(name: &str) -> Option<FileIcon> {
    Some(match name {
        ".gitignore" | ".gitattributes" | ".gitmodules" | ".gitconfig" | ".gitkeep" => GIT,
        ".dockerignore" => DOCKER,
        ".npmrc" => NPM,
        ".yarnrc" | ".yarnrc.yml" => YARN,
        ".pnpmfile.cjs" => PNPM,
        ".prettierrc" | ".prettierrc.json" | ".prettierrc.yml" | ".prettierrc.yaml"
        | ".prettierrc.toml" | ".prettierignore" => PRETTIER,
        ".eslintrc" | ".eslintrc.json" | ".eslintrc.js" | ".eslintrc.cjs" | ".eslintignore" => {
            ESLINT
        }
        ".babelrc" | ".babelrc.json" => BABEL,
        ".editorconfig" => EDITORCONFIG,
        ".vscodeignore" => VSCODE,
        "settings.json" | "keybindings.json" => SETTINGS,
        "angular.json" => ANGULAR,
        "routes.rb" | "schema.rb" => RAILS,
        "manage.py" => DJANGO,
        "wsgi.py" => FLASK,
        "asgi.py" => FASTAPI,
        "application.properties" | "application.yml" | "application.yaml" => SPRING,
        "artisan" => LARAVEL,
        "symfony.lock" => SYMFONY,
        "wp-config.php" => WORDPRESS,
        "jenkinsfile" => JENKINS,
        "action.yml" | "action.yaml" => GITHUB_ACTIONS,
        "firebase.json" | ".firebaserc" => FIREBASE,
        "androidmanifest.xml" => ANDROID,
        "electron-builder.json" => ELECTRON,
        "my.cnf" => MYSQL,
        "pg_hba.conf" => POSTGRESQL,
        "mongod.conf" => MONGODB,
        "redis.conf" => REDIS,
        "pnpm-lock.yaml" => PNPM,
        "yarn.lock" => YARN,
        "npm-shrinkwrap.json" => NPM,
        "cargo.toml" => RUST,
        "cargo.lock" => LOCK,
        "rust-toolchain" | "rust-toolchain.toml" => RUST,
        "dockerfile"
        | "docker-compose.yml"
        | "docker-compose.yaml"
        | "compose.yml"
        | "compose.yaml" => DOCKER,
        "makefile" | "gnumakefile" | "bsdmakefile" => MAKE,
        "cmakelists.txt" => CMAKE,
        "package.json" | "package-lock.json" => NPM,
        "tsconfig.json" | "tsconfig.base.json" => TYPESCRIPT,
        "jsconfig.json" => JAVASCRIPT,
        "deno.json" | "deno.jsonc" | "deno.lock" => DENO,
        "bun.lock" | "bun.lockb" => BUN,
        "go.mod" | "go.sum" | "go.work" => GO,
        "gemfile" | "gemfile.lock" | "rakefile" | "rakefile.rb" => RUBY,
        "composer.json" | "composer.lock" => PHP,
        "build.gradle"
        | "build.gradle.kts"
        | "settings.gradle"
        | "settings.gradle.kts"
        | "gradle.properties"
        | "gradlew"
        | "gradlew.bat" => GRADLE,
        "pom.xml" => MAVEN,
        "mix.exs" | "mix.lock" => ELIXIR,
        "project.clj" | "deps.edn" | "build.clj" => CLOJURE,
        "stack.yaml" | "cabal.project" | "package.yaml" => HASKELL,
        "pubspec.yaml" | "pubspec.lock" => DART,
        "flake.nix" | "default.nix" | "shell.nix" => NIXOS,
        "tauri.conf.json" => TAURI,
        "project.godot" => GODOT,
        "project.pbxproj" => APPLE,
        "build" | "build.bazel" | "workspace" | "workspace.bazel" | "module.bazel" => BAZEL,
        "pipfile" | "pipfile.lock" | "pyproject.toml" | "poetry.lock" | "requirements.txt"
        | "setup.py" | "setup.cfg" | "tox.ini" => PYTHON,
        "license" | "license.txt" | "license.md" | "licence" | "licence.txt" | "licence.md"
        | "copying" | "copying.txt" | "notice" | "notice.txt" => LICENSE,
        _ => return None,
    })
}

fn special_prefix(name: &str) -> Option<FileIcon> {
    if name.starts_with(".env") {
        return Some(CONFIG);
    }
    if name.starts_with("vite.config.") {
        return Some(VITE);
    }
    if name.starts_with("webpack.config.") {
        return Some(WEBPACK);
    }
    if name.starts_with("rollup.config.") {
        return Some(ROLLUP);
    }
    if name.starts_with("babel.config.") {
        return Some(BABEL);
    }
    if name.starts_with("eslint.config.") {
        return Some(ESLINT);
    }
    if name.starts_with("prettier.config.") {
        return Some(PRETTIER);
    }
    if name.starts_with("jest.config.") {
        return Some(JEST);
    }
    if name.starts_with("vitest.config.") {
        return Some(VITEST);
    }
    if name.starts_with("cypress.config.") {
        return Some(CYPRESS);
    }
    if name.starts_with("playwright.config.") {
        return Some(PLAYWRIGHT);
    }
    if name.starts_with("next.config.") {
        return Some(NEXTJS);
    }
    if name.starts_with("nuxt.config.") {
        return Some(NUXT);
    }
    if name.starts_with("remix.config.") {
        return Some(REMIX);
    }
    if name.starts_with("tailwind.config.") {
        return Some(TAILWIND);
    }
    if name.starts_with("postcss.config.") {
        return Some(POSTCSS);
    }
    if name.starts_with("gulpfile.") || name == "gulpfile" {
        return Some(GULP);
    }
    if name.starts_with("gruntfile.") || name == "gruntfile" {
        return Some(GRUNT);
    }
    if name.starts_with("tsconfig.") && name.ends_with(".json") {
        return Some(TYPESCRIPT);
    }
    if name.starts_with("jsconfig.") && name.ends_with(".json") {
        return Some(JAVASCRIPT);
    }
    if name.starts_with("docker-compose.") || name.starts_with("compose.") {
        return Some(DOCKER);
    }
    if name.starts_with("dockerfile") {
        return Some(DOCKER);
    }
    if name.starts_with("makefile") {
        return Some(MAKE);
    }
    if name.starts_with(".gitlab-ci") {
        return Some(GITLAB);
    }
    if name.starts_with(".mocharc") {
        return Some(MOCHA);
    }
    if name.starts_with("jquery.") {
        return Some(JQUERY);
    }
    if name.starts_with("bootstrap.") {
        return Some(BOOTSTRAP);
    }
    if name.starts_with("electron.vite.config.") {
        return Some(ELECTRON);
    }
    if name.contains(".stories.") {
        return Some(STORYBOOK);
    }
    if name.starts_with("requirements") && name.ends_with(".txt") {
        return Some(PYTHON);
    }
    if name.starts_with("readme") {
        return Some(README);
    }
    if name.starts_with("changelog")
        || name.starts_with("contributing")
        || name.starts_with("code_of_conduct")
    {
        return Some(MARKDOWN);
    }
    if name.starts_with("license")
        || name.starts_with("licence")
        || name.starts_with("copying")
        || name.starts_with("notice")
    {
        return Some(LICENSE);
    }
    None
}

fn extension_icon(extension: &str) -> Option<FileIcon> {
    Some(match extension {
        "rs" => RUST,
        "py" | "pyi" | "pyw" | "pyx" | "pyc" | "pyo" => PYTHON,
        "ipynb" => JUPYTER,
        "js" | "mjs" | "cjs" => JAVASCRIPT,
        "jsx" => REACT,
        "ts" | "mts" | "cts" => TYPESCRIPT,
        "tsx" => REACT,
        "html" | "htm" | "xhtml" => HTML,
        "css" => CSS,
        "scss" | "sass" => SASS,
        "less" => LESS,
        "styl" | "stylus" => STYLUS,
        "vue" => VUE,
        "svelte" => SVELTE,
        "astro" => ASTRO,
        "elm" => ELM,
        "json" | "jsonc" | "json5" | "jsonl" | "ndjson" | "map" => JSON,
        "md" | "markdown" | "mdx" | "rst" => MARKDOWN,
        "toml" => TOML,
        "yaml" | "yml" => YAML,
        "xml" | "xsl" | "xslt" | "plist" | "props" | "targets" => XML,
        "svg" => SVG,
        "c" | "h" => C,
        "cc" | "cpp" | "cxx" | "c++" | "hpp" | "hh" | "hxx" | "h++" => CPP,
        "cs" | "csx" => CSHARP,
        "java" | "class" | "jar" => JAVA,
        "kt" | "kts" => KOTLIN,
        "go" => GO,
        "rb" | "erb" | "gemspec" | "rake" | "ru" => RUBY,
        "php" | "phtml" => PHP,
        "swift" => SWIFT,
        "lua" => LUA,
        "dart" => DART,
        "scala" | "sc" => SCALA,
        "hs" | "lhs" => HASKELL,
        "ex" | "exs" => ELIXIR,
        "erl" | "hrl" => ERLANG,
        "clj" | "cljs" | "cljc" | "edn" => CLOJURE,
        "jl" => JULIA,
        "zig" => ZIG,
        "nim" | "nims" => NIM,
        "pl" | "pm" | "pod" => PERL,
        "ps1" | "psm1" | "psd1" => POWERSHELL,
        "sh" | "bash" | "zsh" | "fish" | "ksh" => SHELL,
        "bat" | "cmd" => WINDOWS,
        "r" | "rmd" => R,
        "fs" | "fsi" | "fsx" => FSHARP,
        "ml" | "mli" => OCAML,
        "cr" => CRYSTAL,
        "hx" => HAXE,
        "vala" | "vapi" => VALA,
        "f" | "f90" | "f95" | "f03" | "for" => FORTRAN,
        "m" | "mm" => OBJECTIVEC,
        "mlx" => MATLAB,
        "sol" => SOLIDITY,
        "groovy" | "gvy" => GROOVY,
        "gsp" => GRAILS,
        "vb" => DOTNET,
        "asm" | "s" => ASSEMBLY,
        "pas" | "pp" | "dpr" => DELPHI,
        "tex" | "latex" | "sty" | "cls" | "bib" => LATEX,
        "sql" => SQL,
        "db" | "sqlite" | "sqlite3" => SQLITE,
        "mysql" => MYSQL,
        "pgsql" | "psql" => POSTGRESQL,
        "mongodb" => MONGODB,
        "redis" => REDIS,
        "graphql" | "gql" => GRAPHQL,
        "prisma" => PRISMA,
        "proto" => GRPC,
        "tf" | "tfvars" | "hcl" => TERRAFORM,
        "dockerfile" => DOCKER,
        "kube" | "k8s" => KUBERNETES,
        "helm" => HELM,
        "conf" | "cfg" | "ini" | "config" | "properties" | "env" => CONFIG,
        "nginx" => NGINX,
        "apache" | "htaccess" => APACHE,
        "wasm" | "wat" => WASM,
        "cmake" => CMAKE,
        "mk" | "mak" | "ninja" => MAKE,
        "gradle" => GRADLE,
        "mvn" => MAVEN,
        "bazel" | "bzl" => BAZEL,
        "lock" | "lockb" => LOCK,
        "pem" | "key" | "crt" | "cer" | "p12" | "pfx" | "gpg" | "asc" => LOCK,
        "pdf" => PDF,
        "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" | "zst" | "lz4" | "cab"
        | "iso" => ARCHIVE,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "tif" | "tiff" | "avif"
        | "heic" | "raw" => IMAGE,
        "mp3" | "wav" | "ogg" | "flac" | "m4a" | "aac" | "opus" | "mid" | "midi" => AUDIO,
        "mp4" | "mkv" | "mov" | "avi" | "webm" | "wmv" | "flv" | "m4v" => VIDEO,
        "ttf" | "otf" | "woff" | "woff2" | "eot" => FONT,
        "txt" | "log" | "text" => TEXT,
        "csv" | "tsv" => CSV,
        "doc" | "docx" | "odt" | "rtf" => WORD,
        "xls" | "xlsx" | "ods" => EXCEL,
        "psd" => PHOTOSHOP,
        "ai" => ILLUSTRATOR,
        "blend" => BLENDER,
        "unity" | "prefab" | "asset" => UNITY,
        "uproject" => UNREAL,
        "godot" => GODOT,
        "pro" | "ui" | "qrc" => QT,
        "xcconfig" => APPLE,
        "desktop" | "service" | "socket" | "timer" => LINUX,
        "nix" => NIXOS,
        "csproj" | "fsproj" | "vbproj" | "sln" => DOTNET,
        "vsix" | "code-workspace" => VSCODE,
        "node" => NODE,
        "crx" => CHROME,
        "xpi" => FIREFOX,
        "apk" | "aab" => ANDROID,
        "exe" | "dll" | "so" | "dylib" | "o" | "obj" | "a" | "lib" | "bin" | "dat" => BINARY,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_languages_by_extension() {
        assert_eq!(for_name("main.rs"), RUST);
        assert_eq!(for_name("script.py"), PYTHON);
        assert_eq!(for_name("app.tsx"), REACT);
        assert_eq!(for_name("lib.ts"), TYPESCRIPT);
        assert_eq!(for_name("main.go"), GO);
        assert_eq!(for_name("style.scss"), SASS);
        assert_eq!(for_name("README.md"), README);
        assert_eq!(for_name("photo.PNG"), IMAGE);
    }

    #[test]
    fn maps_special_filenames_before_extensions() {
        assert_eq!(for_name("Cargo.toml"), RUST);
        assert_eq!(for_name("Cargo.lock"), LOCK);
        assert_eq!(for_name("package.json"), NPM);
        assert_eq!(for_name("tsconfig.node.json"), TYPESCRIPT);
        assert_eq!(for_name("vite.config.ts"), VITE);
        assert_eq!(for_name("docker-compose.yml"), DOCKER);
        assert_eq!(for_name("Dockerfile.dev"), DOCKER);
        assert_eq!(for_name(".gitignore"), GIT);
        assert_eq!(for_name(".env.local"), CONFIG);
        assert_eq!(for_name("LICENSE-MIT"), LICENSE);
        assert_eq!(for_name("requirements-dev.txt"), PYTHON);
    }

    #[test]
    fn falls_back_to_generic_file_and_folder_icons() {
        assert_eq!(for_name("unknown.zzz"), FILE);
        assert_eq!(for_name("Makefile"), MAKE);
        assert_eq!(for_entry("src", true, false), FOLDER);
        assert_eq!(for_entry("src", true, true), FOLDER_OPEN);
        assert_eq!(for_entry("main.rs", false, false), RUST);
        assert_eq!(for_path(std::path::Path::new("src/main.rs")), RUST);
    }

    #[test]
    fn every_mapped_glyph_exists_in_the_embedded_font() {
        for icon in ALL {
            assert!(
                graphite_render::icons::AVAILABLE_GLYPHS.contains(&icon.glyph),
                "missing glyph U+{:04X} in the embedded icon font",
                icon.glyph as u32
            );
        }
        for glyph in [CHEVRON_RIGHT, CHEVRON_DOWN] {
            assert!(
                graphite_render::icons::AVAILABLE_GLYPHS.contains(&glyph),
                "missing glyph U+{:04X} in the embedded icon font",
                glyph as u32
            );
        }
    }
}
