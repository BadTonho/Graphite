# Compilação e Testes

Para releases em rascunho acionadas por tag e pacotes Windows/Debian/Ubuntu/Fedora/Arch/AppImage, consulte [releases](releases.md).

Plugins nativos, Emmet e seus testes de integração usam Node.js 24+ no build comum do Cargo. A distribuição portátil inclui Node.js 24.22.1 e o host Code OSS para execução de VSIX.

## Requisitos

- Toolchain Rust estável recente (`rustup`).
- GPU/driver compatível com `wgpu` (DirectX 12 ou Vulkan no Windows; Vulkan ou OpenGL no Linux).
- Linux: pacotes de desenvolvimento de X11/Wayland e fontconfig.
- Um compilador C (as gramáticas do tree-sitter compilam código C): MSVC Build Tools no Windows, `gcc`/`clang` no Linux.

## Comandos

```bash
cargo build --workspace
cargo test --workspace
cargo run -p graphite-app -- <pasta>
```

## Ícone do aplicativo

A arte temporária fica em `assets/branding/graphite.png`. Substitua esse PNG e recompile para atualizar todos os ícones gerados. O build ajusta a arte em telas quadradas sem recortar, preserva o canal alfa, incorpora pixels RGBA de 256×256 na janela e gera ICO com vários tamanhos para executáveis Windows. Os arquivos gerados ficam no `OUT_DIR` do Cargo; não é preciso distribuir uma imagem ao lado do executável. Builds Windows exigem o compilador de recursos do Windows SDK (ou `windres` para GNU Windows).

As janelas Linux usam a identidade `graphite`. Para integração ao desktop, instale `packaging/linux/graphite.desktop` em `$XDG_DATA_HOME/applications` e o PNG de origem como `$XDG_DATA_HOME/icons/graphite.png`; o padrão de `$XDG_DATA_HOME` é `~/.local/share`. Mantenha o executável `graphite` no `PATH`. Launchers Wayland obtêm o ícone dessa entrada de desktop, em vez do ícone da janela. Compilar o workspace não instala esses arquivos no desktop do usuário.

`cargo test -p graphite-app --test branding` verifica tamanhos/pixels gerados, entrada inválida, identidade do desktop Linux e (no Windows) o recurso de ícone no executável real compilado. A renderização de janela/barra de tarefas e a execução Linux exigem verificações de plataforma separadas.

## Host de extensões simulado

Compile o workspace antes de executar `cargo run -p graphite-app -- --simulate-extension-host <pasta>`. O auxiliar Rust deve ficar ao lado do Graphite. É desativado por padrão; Node e Code OSS não são necessários. O comando registrado `graphite.extensions.restartHost` o reinicia manualmente. Consulte [Serviços e host simulado](../architecture/services-host.md) para limites do protocolo, exemplos de atalhos e testes de processo. Para testes específicos da aplicação que usam o auxiliar, execute antes `cargo build -p graphite-extension-host`; `cargo test --workspace` compila automaticamente o alvo de integração de processos.

## Convenções

Consulte [Testes de regressão](testing.md) para a matriz de cobertura, comandos de testes específicos, fixture LSP stdio e verificações manuais/de plataforma pendentes.

- Crie ou atualize testes Rust para comportamentos novos ou alterados e correções de bugs, incluindo casos de falha relevantes. Isole os dados de teste e evite dependência de arquivos pessoais, serviços de rede e servidores de linguagem externos.
- Execute os testes relevantes e `cargo test --workspace` antes de concluir alterações de código; use `cargo build --workspace` para validar a compilação. Informe verificações que falharam ou não foram executadas, o motivo e a plataforma testada. Alterações apenas de documentação não precisam de novos testes, salvo quando alterarem comportamento executável.
- Os testes nativos são executados localmente no sistema disponível. Verifique manualmente interações de janela ou GPU que não puderem ser automatizadas com confiabilidade e informe o que permanecer pendente.
- Mantenha os crates focados; divida um crate quando ele misturar responsabilidades.
- Atualize a documentação em `docs/en/` **e** o arquivo correspondente em `docs/pt-BR/` na mesma mudança.
- Suporte Windows e Linux; use `#[cfg(...)]` quando código específico de plataforma for inevitável.
- Nunca faça commit de segredos (veja a [política de segurança](../security/policy.md)).

## Pacotes portáteis e testes do Code OSS

No Windows ou Linux x86-64, rode `python packaging/portable.py` na raiz do projeto com Python 3.11+, Rust e um compilador C++ nativo. No Windows, instale o Visual Studio Build Tools com desenvolvimento para desktop em C++, o SDK do Windows e as bibliotecas MSVC correspondentes com mitigação Spectre. No Linux, instale um compilador C++ e o Make. O script verifica os arquivos fixados do código-fonte e do Node, instala as dependências fixadas do upstream, compila o host e grava o pacote da plataforma em `target/portable/`. O build comum do Cargo dispensa o Node e os downloads do Code OSS.

Extraia o pacote e execute o Graphite no diretório extraído. Mantenha o diretório `code-oss` ao lado do executável. Para executar o teste de integração com VSIX local, aponte `GRAPHITE_TEST_CODE_OSS` para esse diretório e rode `cargo test -p graphite-app official_host_runs_local_vsix_services_and_invalidates_sessions -- --ignored`. O teste separado `open_vsx_sort_command_edits_and_undoes` também exige `GRAPHITE_TEST_OPEN_VSX_SAMPLE` apontando para o VSIX fixado na [validação da v0.5](../features/v0.5-extensions-automation.md). Por padrão, os testes usam arquivos locais. Defina `GRAPHITE_TEST_OPEN_VSX_LIVE=1` para verificar também a busca e o download no registro público, comparando com o arquivo fixado.
