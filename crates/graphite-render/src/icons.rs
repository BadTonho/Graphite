//! Embedded icon font used for file-type icons.
//!
//! The font is a subset of Nerd Fonts Symbols Only. Only the glyphs listed in
//! [`AVAILABLE_GLYPHS`] are present; `assets/generate-icons.py` rebuilds the
//! subset from this list. Keep the two in sync when adding an icon.

/// Family name assigned to the embedded subset.
pub const FAMILY: &str = "Graphite Icons";

/// Every glyph included in `assets/graphite-icons.ttf`.
///
/// The generator reads the `\u{...}` entries from this list, so adding a glyph
/// here is enough to make it part of the next font rebuild.
pub const AVAILABLE_GLYPHS: &[char] = &[
    '\u{e5ff}', // custom-folder
    '\u{e5fe}', // custom-folder_open
    '\u{ea7b}', // cod-file
    '\u{ec5e}', // cod-file_text
    '\u{eab6}', // cod-chevron_right
    '\u{eab4}', // cod-chevron_down
    '\u{e7a8}', // dev-rust
    '\u{e73c}', // dev-python
    '\u{e74e}', // dev-javascript_alt
    '\u{e8ca}', // dev-typescript
    '\u{e625}', // seti-react
    '\u{e736}', // dev-html5
    '\u{e749}', // dev-css3
    '\u{e74b}', // dev-sass
    '\u{e758}', // dev-less
    '\u{e759}', // dev-stylus
    '\u{e8dc}', // dev-vuejs
    '\u{e8b7}', // dev-svelte
    '\u{e735}', // dev-astro
    '\u{e753}', // dev-angular
    '\u{e80b}', // dev-json
    '\u{e73e}', // dev-markdown
    '\u{eda4}', // fa-readme
    '\u{e6b2}', // custom-toml
    '\u{e8eb}', // dev-yaml
    '\u{e619}', // seti-xml
    '\u{e698}', // seti-svg
    '\u{e771}', // dev-c_lang
    '\u{e7a3}', // dev-cplusplus
    '\u{e7b2}', // dev-csharp
    '\u{e738}', // dev-java
    '\u{e81b}', // dev-kotlin
    '\u{e724}', // dev-go
    '\u{e739}', // dev-ruby
    '\u{e73b}', // dev-ruby_on_rails
    '\u{e73d}', // dev-php
    '\u{e755}', // dev-swift
    '\u{e620}', // seti-lua
    '\u{e64c}', // seti-dart
    '\u{e68e}', // seti-scala
    '\u{e777}', // dev-haskell
    '\u{e7cd}', // dev-elixir
    '\u{e7b1}', // dev-erlang
    '\u{e768}', // dev-clojure
    '\u{e624}', // seti-julia
    '\u{e6a9}', // seti-zig
    '\u{e677}', // seti-nim
    '\u{e769}', // dev-perl
    '\u{e86c}', // dev-powershell
    '\u{e760}', // dev-bash
    '\u{e70f}', // dev-windows
    '\u{e881}', // dev-r
    '\u{e7a7}', // dev-fsharp
    '\u{e84e}', // dev-ocaml
    '\u{e7ce}', // dev-elm
    '\u{e7ac}', // dev-crystal
    '\u{e7fa}', // dev-haxe
    '\u{e8d1}', // dev-vala
    '\u{e7de}', // dev-fortran
    '\u{e82a}', // dev-matlab
    '\u{e8a6}', // dev-solidity
    '\u{e775}', // dev-groovy
    '\u{e7b3}', // dev-grails
    '\u{e77f}', // dev-dotnet
    '\u{e637}', // seti-asm
    '\u{e904}', // dev-delphi
    '\u{e81f}', // dev-latex
    '\u{e80f}', // dev-jupyter
    '\u{e84d}', // dev-objectivec
    '\u{e706}', // dev-database
    '\u{e704}', // dev-mysql
    '\u{e76e}', // dev-postgresql
    '\u{e7c4}', // dev-sqlite
    '\u{e7a4}', // dev-mongodb
    '\u{e76d}', // dev-redis
    '\u{e7f4}', // dev-graphql
    '\u{e86e}', // dev-prisma
    '\u{e7f5}', // dev-grpc
    '\u{e69a}', // seti-terraform
    '\u{e7b0}', // dev-docker
    '\u{e81d}', // dev-kubernetes
    '\u{e7fb}', // dev-helm
    '\u{e776}', // dev-nginx
    '\u{e72b}', // dev-apache
    '\u{e702}', // dev-git
    '\u{e71e}', // dev-npm
    '\u{e719}', // dev-nodejs
    '\u{e7c0}', // dev-denojs
    '\u{e8e0}', // dev-wasm
    '\u{e794}', // dev-cmake
    '\u{e673}', // seti-makefile
    '\u{e7f2}', // dev-gradle
    '\u{e82c}', // dev-maven
    '\u{e8f9}', // dev-bazel
    '\u{e71d}', // dev-django
    '\u{e7dc}', // dev-flask
    '\u{e7d5}', // dev-fastapi
    '\u{e8ac}', // dev-spring
    '\u{e73f}', // dev-laravel
    '\u{e757}', // dev-symfony_badge
    '\u{e70b}', // dev-wordpress
    '\u{e750}', // dev-jquery
    '\u{e747}', // dev-bootstrap
    '\u{e8ba}', // dev-tailwindcss
    '\u{e86a}', // dev-postcss
    '\u{e8d6}', // dev-vite
    '\u{e8e3}', // dev-webpack
    '\u{e892}', // dev-rollup
    '\u{e75d}', // dev-babel
    '\u{e763}', // dev-gulp
    '\u{e74c}', // dev-grunt
    '\u{e7d2}', // dev-eslint
    '\u{e6b4}', // custom-prettier
    '\u{e807}', // dev-jest
    '\u{e832}', // dev-mocha
    '\u{e8d9}', // dev-vitest
    '\u{e7b9}', // dev-cypressio
    '\u{e863}', // dev-playwright
    '\u{e8b3}', // dev-storybook
    '\u{e8ec}', // dev-yarn
    '\u{e865}', // dev-pnpm
    '\u{e76f}', // dev-bun
    '\u{e7eb}', // dev-gitlab
    '\u{e7e9}', // dev-githubactions
    '\u{e767}', // dev-jenkins
    '\u{e787}', // dev-firebase
    '\u{e712}', // dev-linux
    '\u{e711}', // dev-apple
    '\u{e70e}', // dev-android
    '\u{e743}', // dev-chrome
    '\u{e745}', // dev-firefox
    '\u{e83e}', // dev-nextjs
    '\u{e931}', // dev-nuxt
    '\u{e93e}', // dev-remix
    '\u{e843}', // dev-nixos
    '\u{e8bb}', // dev-tauri
    '\u{e7cb}', // dev-electron
    '\u{e87d}', // dev-qt
    '\u{e721}', // dev-unity_small
    '\u{e7ee}', // dev-godot
    '\u{e8cd}', // dev-unrealengine
    '\u{e7b8}', // dev-photoshop
    '\u{e7b4}', // dev-illustrator
    '\u{e766}', // dev-blender
    '\u{e8da}', // dev-vscode
    '\u{eae8}', // cod-file_binary
    '\u{e672}', // seti-lock
    '\u{e67d}', // seti-pdf
    '\u{e6aa}', // seti-zip
    '\u{e60d}', // seti-image
    '\u{e638}', // seti-audio
    '\u{e69f}', // seti-video
    '\u{e659}', // seti-font
    '\u{e64e}', // seti-text
    '\u{e60a}', // seti-license
    '\u{e615}', // seti-config
    '\u{e652}', // seti-editorconfig
    '\u{e690}', // seti-settings
    '\u{e6a5}', // seti-word
    '\u{e6a6}', // seti-xls
    '\u{e64a}', // seti-csv
];

pub(crate) const FONT: &[u8] = include_bytes!("../assets/graphite-icons.ttf");

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic_text::{
        Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache, SwashContent,
    };
    use std::sync::Arc;

    #[test]
    fn every_advertised_glyph_rasterizes_from_the_embedded_font() {
        let source = cosmic_text::fontdb::Source::Binary(Arc::new(FONT));
        let mut font_system = FontSystem::new_with_fonts([source]);
        let mut swash = SwashCache::new();
        let metrics = Metrics::new(16.0, 16.0);

        for &glyph in AVAILABLE_GLYPHS {
            let mut buffer = Buffer::new(&mut font_system, metrics);
            buffer.set_size(None, None);
            buffer.set_text(
                &glyph.to_string(),
                &Attrs::new().family(Family::Name(FAMILY)),
                Shaping::Advanced,
                None,
            );
            buffer.shape_until_scroll(&mut font_system, false);

            let key = buffer
                .layout_runs()
                .flat_map(|run| run.glyphs.iter())
                .map(|glyph| glyph.physical((0.0, 0.0), 1.0).cache_key)
                .next()
                .unwrap_or_else(|| panic!("missing layout glyph for U+{:04X}", glyph as u32));
            let image = swash
                .get_image_uncached(&mut font_system, key)
                .unwrap_or_else(|| panic!("missing rasterized glyph for U+{:04X}", glyph as u32));
            assert!(
                image.placement.width > 0 && image.placement.height > 0,
                "empty rasterized glyph for U+{:04X}",
                glyph as u32
            );
            assert!(matches!(
                image.content,
                SwashContent::Mask | SwashContent::Color | SwashContent::SubpixelMask
            ));
        }
    }
}
