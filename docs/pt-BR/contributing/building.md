# Compilação e Testes

Plugins nativos e seus testes de integração exigem Node.js 24+. O editor e a inspeção de VSIX podem funcionar sem Node; consulte [Desenvolvimento de plugins](plugins.md).

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

